use super::{
    block::Header,
    BlockId,
    Bytes32,
    HexString,
    PageDirection,
    PageInfo,
    PaginatedResult,
    PaginationRequest,
    TransactionId,
};
use crate::client::schema::{
    schema,
    Address,
    MessageId,
    Nonce,
    U32,
    U64,
};

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct Message {
    pub amount: U64,
    pub sender: Address,
    pub recipient: Address,
    pub nonce: Nonce,
    pub data: HexString,
    pub da_height: U64,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Query",
    variables = "OwnedMessagesConnectionArgs"
)]
pub struct OwnedMessageQuery {
    #[arguments(owner: $owner, after: $after, before: $before, first: $first, last: $last)]
    pub messages: MessageConnection,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct MessageConnection {
    pub edges: Vec<MessageEdge>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct MessageEdge {
    pub cursor: String,
    pub node: Message,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct OwnedMessagesConnectionArgs {
    /// Filter messages based on an owner
    pub owner: Option<Address>,
    /// Skip until coin id (forward pagination)
    pub after: Option<String>,
    /// Skip until coin id (backward pagination)
    pub before: Option<String>,
    /// Retrieve the first n coins in order (forward pagination)
    pub first: Option<i32>,
    /// Retrieve the last n coins in order (backward pagination).
    /// Can't be used at the same time as `first`.
    pub last: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Query",
    variables = "MessageProofArgs"
)]
pub struct MessageProofQuery {
    #[arguments(
        transactionId: $transaction_id,
        messageId: $message_id,
        commitBlockId: $commit_block_id,
        commitBlockHeight: $commit_block_height
    )]
    pub message_proof: Option<MessageProof>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct MerkleProof {
    /// The proof set of the message proof.
    pub proof_set: Vec<Bytes32>,
    /// The index that was used to produce this proof.
    pub proof_index: U64,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct MessageProof {
    /// Proof that message is contained within the provided block header.
    pub message_proof: MerkleProof,
    /// Proof that the provided block header is contained within the blockchain history.
    pub block_proof: MerkleProof,
    /// The previous fuel block header that contains the message. Message block height <
    /// commit block height.
    pub message_block_header: Header,
    /// The consensus header associated with the finalized commit being used
    /// as the root of the block proof.
    pub commit_block_header: Header,

    /// The messages sender address.
    pub sender: Address,
    /// The messages recipient address.
    pub recipient: Address,
    /// The nonce from the message.
    pub nonce: Nonce,
    /// The amount from the message.
    pub amount: U64,
    /// The data from the message.
    pub data: HexString,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct MessageProofArgs {
    /// Transaction id that contains the output message.
    pub transaction_id: TransactionId,
    /// Message id of the output message that requires a proof.
    pub message_id: MessageId,

    /// The query supports either `commit_block_id`, or `commit_block_height` set on, not both.

    /// The block id of the commitment block.
    /// If it is `None`, the `commit_block_height` should be `Some`.
    pub commit_block_id: Option<BlockId>,
    /// The block height of the commitment block.
    /// If it is `None`, the `commit_block_id` should be `Some`.
    pub commit_block_height: Option<U32>,
}

impl From<(Option<Address>, PaginationRequest<String>)> for OwnedMessagesConnectionArgs {
    fn from(r: (Option<Address>, PaginationRequest<String>)) -> Self {
        match r.1.direction {
            PageDirection::Forward => OwnedMessagesConnectionArgs {
                owner: r.0,
                after: r.1.cursor,
                before: None,
                first: Some(r.1.results as i32),
                last: None,
            },
            PageDirection::Backward => OwnedMessagesConnectionArgs {
                owner: r.0,
                after: None,
                before: r.1.cursor,
                first: None,
                last: Some(r.1.results as i32),
            },
        }
    }
}

impl From<MessageConnection> for PaginatedResult<Message, String> {
    fn from(conn: MessageConnection) -> Self {
        PaginatedResult {
            cursor: conn.page_info.end_cursor,
            has_next_page: conn.page_info.has_next_page,
            has_previous_page: conn.page_info.has_previous_page,
            results: conn.edges.into_iter().map(|e| e.node).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_message_query_gql_output() {
        use cynic::QueryBuilder;

        let operation = OwnedMessageQuery::build(OwnedMessagesConnectionArgs {
            owner: Some(Address::default()),
            after: None,
            before: None,
            first: None,
            last: None,
        });

        insta::assert_snapshot!(operation.query)
    }
}
