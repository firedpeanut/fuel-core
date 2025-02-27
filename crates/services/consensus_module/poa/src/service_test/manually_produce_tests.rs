use fuel_core_types::{
    blockchain::block::Block,
    tai64::Tai64,
};
use test_case::test_case;

use super::*;

#[test_case(Tai64::now(), 10, vec![Tai64::now(); 10], Trigger::Never, 0)]
#[test_case(Tai64::now(), 10, vec![Tai64::now(); 10], Trigger::Instant, 0)]
#[test_case(
    Tai64::now(), 3, vec![Tai64::now(), Tai64::now() + 10, Tai64::now() + 20],
    Trigger::Interval { block_time: Duration::from_secs(10) }, 0
)]
#[test_case(Tai64::now() + 100, 10, vec![Tai64::now() + 100; 10], Trigger::Never, 0)]
#[test_case(Tai64::now() + 100, 10, vec![Tai64::now() + 100; 10], Trigger::Instant, 0)]
#[test_case(
    Tai64::now() + 100, 3, vec![Tai64::now() + 100, Tai64::now() + 110, Tai64::now() + 120],
    Trigger::Interval { block_time: Duration::from_secs(10) }, 0
)]
#[test_case(Tai64::now(), 10, vec![Tai64::now(); 10], Trigger::Never, 10)]
#[test_case(Tai64::now(), 10, vec![Tai64::now(); 10], Trigger::Instant, 10)]
#[test_case(
    Tai64::now(), 3, vec![Tai64::now(), Tai64::now() + 10, Tai64::now() + 20],
    Trigger::Interval { block_time: Duration::from_secs(10) }, 10
)]
#[test_case(Tai64::now() + 100, 10, vec![Tai64::now() + 100; 10], Trigger::Never, 10)]
#[test_case(Tai64::now() + 100, 10, vec![Tai64::now() + 100; 10], Trigger::Instant, 10)]
#[test_case(
    Tai64::now() + 100, 3, vec![Tai64::now() + 100, Tai64::now() + 110, Tai64::now() + 120],
    Trigger::Interval { block_time: Duration::from_secs(10) }, 10
)]
#[tokio::test]
async fn can_manually_produce_block(
    start_time: Tai64,
    number_of_blocks: u32,
    times: Vec<Tai64>,
    trigger: Trigger,
    num_txns: usize,
) {
    let mut rng = StdRng::seed_from_u64(1234u64);
    let mut ctx_builder = TestContextBuilder::new();
    ctx_builder.with_config(Config {
        trigger,
        block_gas_limit: 100_000,
        signing_key: Some(test_signing_key()),
        metrics: false,
        consensus_params: Default::default(),
    });

    // initialize txpool with some txs
    let TxPoolContext {
        txpool,
        status_sender,
        ..
    } = MockTransactionPool::new_with_txs(
        (0..num_txns).map(|_| make_tx(&mut rng)).collect(),
    );
    ctx_builder.with_txpool(txpool);

    let mut importer = MockBlockImporter::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(times.len());
    importer.expect_commit_result().returning(move |r| {
        tx.try_send(r.into_result().sealed_block.entity.header().time())
            .unwrap();
        Ok(())
    });
    let mut producer = MockBlockProducer::default();
    producer
        .expect_produce_and_execute_block()
        .returning(|_, time, _| {
            let mut block = Block::default();
            block.header_mut().consensus.time = time;
            block.header_mut().recalculate_metadata();
            Ok(UncommittedResult::new(
                ExecutionResult {
                    block,
                    skipped_transactions: Default::default(),
                    tx_status: Default::default(),
                },
                StorageTransaction::new(EmptyStorage),
            ))
        });
    ctx_builder.with_importer(importer);
    ctx_builder.with_producer(producer);
    let ctx = ctx_builder.build();

    ctx.service
        .shared
        .manually_produce_block(Some(start_time), number_of_blocks)
        .await
        .unwrap();
    for _ in 0..num_txns {
        status_sender.send_replace(Some(TxStatus::Submitted));
    }

    for t in times.into_iter() {
        let block_time = rx.recv().await.unwrap();
        assert_eq!(t, block_time);
    }

    // Stop
    assert_eq!(ctx.stop().await, State::Stopped);
}
