use crate::{
    new_service,
    ports::{
        MockBlockImporter,
        MockBlockProducer,
        MockTransactionPool,
    },
    service::Task,
    Config,
    Service,
    Trigger,
};
use fuel_core_services::{
    stream::pending,
    Service as StorageTrait,
    State,
};
use fuel_core_storage::{
    test_helpers::EmptyStorage,
    transactional::StorageTransaction,
};
use fuel_core_types::{
    blockchain::{
        header::BlockHeader,
        primitives::SecretKeyWrapper,
        SealedBlock,
    },
    fuel_asm::*,
    fuel_crypto::SecretKey,
    fuel_tx::{
        field::GasLimit,
        *,
    },
    fuel_types::BlockHeight,
    secrecy::Secret,
    services::{
        executor::{
            Error as ExecutorError,
            ExecutionResult,
            UncommittedResult,
        },
        txpool::{
            Error as TxPoolError,
            TxStatus,
        },
    },
    tai64::Tai64,
};
use rand::{
    prelude::StdRng,
    Rng,
    SeedableRng,
};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        Mutex as StdMutex,
        Mutex,
    },
    time::Duration,
};
use tokio::{
    sync::{
        broadcast,
        watch,
    },
    time,
};

mod manually_produce_tests;
mod trigger_tests;

struct TestContextBuilder {
    config: Option<Config>,
    txpool: Option<MockTransactionPool>,
    importer: Option<MockBlockImporter>,
    producer: Option<MockBlockProducer>,
}

impl TestContextBuilder {
    fn new() -> Self {
        Self {
            config: None,
            txpool: None,
            importer: None,
            producer: None,
        }
    }

    fn with_config(&mut self, config: Config) -> &mut Self {
        self.config = Some(config);
        self
    }

    fn with_txpool(&mut self, txpool: MockTransactionPool) -> &mut Self {
        self.txpool = Some(txpool);
        self
    }

    fn with_importer(&mut self, importer: MockBlockImporter) -> &mut Self {
        self.importer = Some(importer);
        self
    }

    fn with_producer(&mut self, producer: MockBlockProducer) -> &mut Self {
        self.producer = Some(producer);
        self
    }

    fn build(self) -> TestContext {
        let config = self.config.unwrap_or_default();
        let producer = self.producer.unwrap_or_else(|| {
            let mut producer = MockBlockProducer::default();
            producer
                .expect_produce_and_execute_block()
                .returning(|_, _, _| {
                    Ok(UncommittedResult::new(
                        ExecutionResult {
                            block: Default::default(),
                            skipped_transactions: Default::default(),
                            tx_status: Default::default(),
                        },
                        StorageTransaction::new(EmptyStorage),
                    ))
                });
            producer
        });

        let importer = self.importer.unwrap_or_else(|| {
            let mut importer = MockBlockImporter::default();
            importer.expect_commit_result().returning(|_| Ok(()));
            importer
        });

        let txpool = self
            .txpool
            .unwrap_or_else(MockTransactionPool::no_tx_updates);

        let service = new_service(
            &BlockHeader::new_block(BlockHeight::from(1u32), Tai64::now()),
            config,
            txpool,
            producer,
            importer,
        );
        service.start().unwrap();
        TestContext { service }
    }
}

struct TestContext {
    service: Service<MockTransactionPool, MockBlockProducer, MockBlockImporter>,
}

impl TestContext {
    async fn stop(&self) -> State {
        self.service.stop_and_await().await.unwrap()
    }
}

pub struct TxPoolContext {
    pub txpool: MockTransactionPool,
    pub txs: Arc<Mutex<Vec<Script>>>,
    pub status_sender: Arc<watch::Sender<Option<TxStatus>>>,
}

impl MockTransactionPool {
    fn no_tx_updates() -> Self {
        let mut txpool = MockTransactionPool::default();
        txpool
            .expect_transaction_status_events()
            .returning(|| Box::pin(pending()));
        txpool
    }

    pub fn new_with_txs(txs: Vec<Script>) -> TxPoolContext {
        let mut txpool = MockTransactionPool::default();
        let txs = Arc::new(StdMutex::new(txs));
        let (status_sender, status_receiver) = watch::channel(None);
        let status_sender = Arc::new(status_sender);
        let status_sender_clone = status_sender.clone();

        txpool
            .expect_transaction_status_events()
            .returning(move || {
                let status_channel =
                    (status_sender_clone.clone(), status_receiver.clone());
                let stream = fuel_core_services::stream::unfold(
                    status_channel,
                    |(sender, mut receiver)| async {
                        loop {
                            let status = receiver.borrow_and_update().clone();
                            if let Some(status) = status {
                                sender.send_replace(None);
                                return Some((status, (sender, receiver)))
                            }
                            receiver.changed().await.unwrap();
                        }
                    },
                );
                Box::pin(stream)
            });

        let pending = txs.clone();
        txpool
            .expect_pending_number()
            .returning(move || pending.lock().unwrap().len());
        let consumable = txs.clone();
        txpool.expect_total_consumable_gas().returning(move || {
            consumable
                .lock()
                .unwrap()
                .iter()
                .map(|tx| *tx.gas_limit())
                .sum()
        });
        let removed = txs.clone();
        txpool
            .expect_remove_txs()
            .returning(move |tx_ids: Vec<TxId>| {
                let mut guard = removed.lock().unwrap();
                for id in tx_ids {
                    guard.retain(|tx| tx.id(&ConsensusParameters::DEFAULT) == id);
                }
                vec![]
            });

        TxPoolContext {
            txpool,
            txs,
            status_sender,
        }
    }
}

fn make_tx(rng: &mut StdRng) -> Script {
    TransactionBuilder::script(vec![], vec![])
        .gas_price(0)
        .gas_limit(rng.gen_range(1..ConsensusParameters::default().max_gas_per_tx))
        .finalize_without_signature()
}

#[tokio::test]
async fn remove_skipped_transactions() {
    // The test verifies that if `BlockProducer` returns skipped transactions, they would
    // be propagated to `TxPool` for removal.
    let mut rng = StdRng::seed_from_u64(2322);
    let secret_key = SecretKey::random(&mut rng);

    const TX_NUM: usize = 100;
    let skipped_transactions: Vec<_> = (0..TX_NUM).map(|_| make_tx(&mut rng)).collect();

    let mock_skipped_txs = skipped_transactions.clone();

    let mut block_producer = MockBlockProducer::default();
    block_producer
        .expect_produce_and_execute_block()
        .times(1)
        .returning(move |_, _, _| {
            Ok(UncommittedResult::new(
                ExecutionResult {
                    block: Default::default(),
                    skipped_transactions: mock_skipped_txs
                        .clone()
                        .into_iter()
                        .map(|tx| (tx.into(), ExecutorError::OutputAlreadyExists))
                        .collect(),
                    tx_status: Default::default(),
                },
                StorageTransaction::new(EmptyStorage),
            ))
        });

    let mut block_importer = MockBlockImporter::default();

    block_importer
        .expect_commit_result()
        .times(1)
        .returning(|_| Ok(()));

    let mut txpool = MockTransactionPool::no_tx_updates();
    // Test created for only for this check.
    txpool.expect_remove_txs().returning(move |skipped_ids| {
        // Transform transactions into ids.
        let skipped_transactions: Vec<_> = skipped_transactions
            .iter()
            .map(|tx| tx.id(&ConsensusParameters::DEFAULT))
            .collect();

        // Check that all transactions are unique.
        let expected_skipped_ids_set: HashSet<_> =
            skipped_transactions.clone().into_iter().collect();
        assert_eq!(expected_skipped_ids_set.len(), TX_NUM);

        // Check that `TxPool::remove_txs` was called with the same ids in the same order.
        assert_eq!(skipped_ids.len(), TX_NUM);
        assert_eq!(skipped_transactions.len(), TX_NUM);
        assert_eq!(skipped_transactions, skipped_ids);
        vec![]
    });

    let config = Config {
        trigger: Trigger::Instant,
        block_gas_limit: 1000000,
        signing_key: Some(Secret::new(secret_key.into())),
        metrics: false,
        consensus_params: Default::default(),
    };
    let mut task = Task::new(
        &BlockHeader::new_block(BlockHeight::from(1u32), Tai64::now()),
        config,
        txpool,
        block_producer,
        block_importer,
    );

    assert!(task.produce_next_block().await.is_ok());
}

#[tokio::test]
async fn does_not_produce_when_txpool_empty_in_instant_mode() {
    // verify the PoA service doesn't trigger empty blocks to be produced when there are
    // irrelevant updates from the txpool
    let mut rng = StdRng::seed_from_u64(2322);
    let secret_key = SecretKey::random(&mut rng);

    let mut block_producer = MockBlockProducer::default();

    block_producer
        .expect_produce_and_execute_block()
        .returning(|_, _, _| panic!("Block production should not be called"));

    let mut block_importer = MockBlockImporter::default();

    block_importer
        .expect_commit_result()
        .returning(|_| panic!("Block importer should not be called"));

    let mut txpool = MockTransactionPool::no_tx_updates();
    txpool.expect_total_consumable_gas().returning(|| 0);
    txpool.expect_pending_number().returning(|| 0);

    let config = Config {
        trigger: Trigger::Instant,
        block_gas_limit: 1000000,
        signing_key: Some(Secret::new(secret_key.into())),
        metrics: false,
        consensus_params: Default::default(),
    };
    let mut task = Task::new(
        &BlockHeader::new_block(BlockHeight::from(1u32), Tai64::now()),
        config,
        txpool,
        block_producer,
        block_importer,
    );

    // simulate some txpool events to see if any block production is erroneously triggered
    task.on_txpool_event(TxStatus::Submitted).await.unwrap();
    task.on_txpool_event(TxStatus::Completed).await.unwrap();
    task.on_txpool_event(TxStatus::SqueezedOut {
        reason: TxPoolError::NoMetadata,
    })
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
async fn hybrid_production_doesnt_produce_empty_blocks_when_txpool_is_empty() {
    // verify the PoA service doesn't alter the hybrid block timing when
    // receiving txpool events if txpool is actually empty
    let mut rng = StdRng::seed_from_u64(2322);
    let secret_key = SecretKey::random(&mut rng);

    const TX_IDLE_TIME_MS: u64 = 50u64;

    let (txpool_tx, _txpool_broadcast) = broadcast::channel(10);

    let mut block_producer = MockBlockProducer::default();

    block_producer
        .expect_produce_and_execute_block()
        .returning(|_, _, _| panic!("Block production should not be called"));

    let mut block_importer = MockBlockImporter::default();

    block_importer
        .expect_commit_result()
        .returning(|_| panic!("Block importer should not be called"));

    let mut txpool = MockTransactionPool::no_tx_updates();
    txpool.expect_total_consumable_gas().returning(|| 0);
    txpool.expect_pending_number().returning(|| 0);

    let config = Config {
        trigger: Trigger::Hybrid {
            min_block_time: Duration::from_millis(100),
            max_tx_idle_time: Duration::from_millis(TX_IDLE_TIME_MS),
            max_block_time: Duration::from_millis(1000),
        },
        block_gas_limit: 1000000,
        signing_key: Some(Secret::new(secret_key.into())),
        metrics: false,
        consensus_params: Default::default(),
    };
    let task = Task::new(
        &BlockHeader::new_block(BlockHeight::from(1u32), Tai64::now()),
        config,
        txpool,
        block_producer,
        block_importer,
    );

    let service = Service::new(task);
    service.start_and_await().await.unwrap();

    // simulate some txpool events to see if any block production is erroneously triggered
    txpool_tx.send(TxStatus::Submitted).unwrap();
    txpool_tx.send(TxStatus::Completed).unwrap();
    txpool_tx
        .send(TxStatus::SqueezedOut {
            reason: TxPoolError::NoMetadata,
        })
        .unwrap();

    // wait max_tx_idle_time - causes block production to occur if
    // pending txs > 0 is not checked.
    time::sleep(Duration::from_millis(TX_IDLE_TIME_MS)).await;

    service.stop_and_await().await.unwrap();
    assert!(service.state().stopped());
}

fn test_signing_key() -> Secret<SecretKeyWrapper> {
    let mut rng = StdRng::seed_from_u64(0);
    let secret_key = SecretKey::random(&mut rng);
    Secret::new(secret_key.into())
}
