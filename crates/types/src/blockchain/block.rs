//! Types related to blocks

use super::{
    consensus::ConsensusType,
    header::{
        ApplicationHeader,
        BlockHeader,
        ConsensusHeader,
        PartialBlockHeader,
    },
    primitives::{
        BlockId,
        Empty,
    },
};
use crate::{
    fuel_tx::{
        ConsensusParameters,
        Transaction,
        TxId,
        UniqueIdentifier,
    },
    fuel_types::MessageId,
};

/// Fuel block with all transaction data included
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "test-helpers"), derive(Default))]
pub struct Block<TransactionRepresentation = Transaction> {
    /// Generated complete header.
    header: BlockHeader,
    /// Executed transactions.
    transactions: Vec<TransactionRepresentation>,
}

/// Compressed version of the fuel `Block`.
pub type CompressedBlock = Block<TxId>;

/// Fuel block with all transaction data included
/// but without any data generated.
/// This type can be created with unexecuted
/// transactions to produce a [`Block`] or
/// it can be created with pre-executed transactions in
/// order to validate they were constructed correctly.
#[derive(Clone, Debug)]
pub struct PartialFuelBlock {
    /// The partial header.
    pub header: PartialBlockHeader,
    /// Transactions that can either be pre-executed
    /// or not.
    pub transactions: Vec<Transaction>,
}

impl Block<Transaction> {
    /// Create a new full fuel block from a [`PartialBlockHeader`],
    /// executed transactions and the [`MessageId`]s.
    ///
    /// The order of the transactions must be the same order they were
    /// executed in.
    /// The order of the messages must be the same as they were
    /// produced in.
    ///
    /// Message ids are produced by executed the transactions and collecting
    /// the ids from the receipts of messages outputs.
    pub fn new(
        header: PartialBlockHeader,
        transactions: Vec<Transaction>,
        message_ids: &[MessageId],
    ) -> Self {
        Self {
            header: header.generate(&transactions, message_ids),
            transactions,
        }
    }

    /// Try creating a new full fuel block from a [`BlockHeader`] and
    /// **previously executed** transactions.
    /// This will fail if the transactions don't match the header.
    pub fn try_from_executed(
        header: BlockHeader,
        transactions: Vec<Transaction>,
    ) -> Option<Self> {
        header.validate_transactions(&transactions).then_some(Self {
            header,
            transactions,
        })
    }

    /// Compresses the fuel block and replaces transactions with hashes.
    pub fn compress(&self, params: &ConsensusParameters) -> CompressedBlock {
        Block {
            header: self.header.clone(),
            transactions: self.transactions.iter().map(|tx| tx.id(params)).collect(),
        }
    }
}

impl<T> Block<T> {
    /// Destructure into the inner types.
    pub fn into_inner(self) -> (BlockHeader, Vec<T>) {
        (self.header, self.transactions)
    }
}

impl CompressedBlock {
    /// Convert from a compressed block back to a the full block.
    pub fn uncompress(self, transactions: Vec<Transaction>) -> Block<Transaction> {
        // TODO: should we perform an extra validation step to ensure the provided
        //  txs match the expected ones in the block?
        Block {
            header: self.header,
            transactions,
        }
    }
}

impl<TransactionRepresentation> Block<TransactionRepresentation> {
    /// Get the hash of the header.
    pub fn id(&self) -> BlockId {
        // The `Block` can be created only via the `Block::new` method, which calculates the
        // identifier based on the header. So the block is immutable and can't change its
        // identifier on the fly.
        //
        // This assertion is a double-checks that this behavior is not changed.
        debug_assert_eq!(self.header.id(), self.header.hash());
        self.header.id()
    }

    /// Get the executed transactions.
    pub fn transactions(&self) -> &[TransactionRepresentation] {
        &self.transactions[..]
    }

    /// Get the complete header.
    pub fn header(&self) -> &BlockHeader {
        &self.header
    }

    /// The type of consensus this header is using.
    pub fn consensus_type(&self) -> ConsensusType {
        self.header.consensus_type()
    }

    /// Get mutable access to transactions for testing purposes
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn transactions_mut(&mut self) -> &mut Vec<TransactionRepresentation> {
        &mut self.transactions
    }

    /// Get mutable access to header for testing purposes
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn header_mut(&mut self) -> &mut BlockHeader {
        &mut self.header
    }
}

impl PartialFuelBlock {
    /// Create a new block
    pub fn new(header: PartialBlockHeader, transactions: Vec<Transaction>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    /// Generate a [`Block`] after running this partial block.
    ///
    /// The order of the messages must be the same as they were
    /// produced in.
    ///
    /// Message ids are produced by executed the transactions and collecting
    /// the ids from the receipts of messages outputs.
    pub fn generate(self, message_ids: &[MessageId]) -> Block {
        Block::new(self.header, self.transactions, message_ids)
    }
}

impl From<Block> for PartialFuelBlock {
    fn from(block: Block) -> Self {
        let Block {
            header:
                BlockHeader {
                    application: ApplicationHeader { da_height, .. },
                    consensus:
                        ConsensusHeader {
                            prev_root,
                            height,
                            time,
                            ..
                        },
                    ..
                },
            transactions,
        } = block;
        Self {
            header: PartialBlockHeader {
                application: ApplicationHeader {
                    da_height,
                    generated: Empty {},
                },
                consensus: ConsensusHeader {
                    prev_root,
                    height,
                    time,
                    generated: Empty {},
                },
            },
            transactions,
        }
    }
}

#[cfg(any(test, feature = "test-helpers"))]
impl CompressedBlock {
    /// Create a compressed header for testing. This does not generate fields.
    pub fn test(header: BlockHeader, transactions: Vec<TxId>) -> Self {
        Self {
            header,
            transactions,
        }
    }
}
