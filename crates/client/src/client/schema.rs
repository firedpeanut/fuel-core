// this is the format cynic expects
#[allow(clippy::module_inception)]
pub mod schema {
    cynic::use_schema!("./assets/schema.sdl");
}

use fuel_core_types::fuel_tx;
use hex::FromHexError;
use std::{
    array::TryFromSliceError,
    fmt::{
        self,
        Debug,
    },
    io::ErrorKind,
    num::TryFromIntError,
};
use thiserror::Error;

pub use primitives::*;

pub mod balance;
pub mod block;
pub mod chain;
pub mod coins;
pub mod contract;
pub mod message;
pub mod node_info;
pub mod primitives;
pub mod tx;

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl", graphql_type = "Query")]
pub struct Health {
    pub health: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl", graphql_type = "Mutation")]
pub struct StartSession {
    pub start_session: cynic::Id,
}

#[derive(cynic::QueryVariables)]
pub struct IdArg {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "IdArg"
)]
pub struct EndSession {
    #[arguments(id: $id)]
    pub end_session: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "IdArg"
)]
pub struct Reset {
    #[arguments(id: $id)]
    pub reset: bool,
}

#[derive(cynic::QueryVariables)]
pub struct ExecuteArgs {
    pub id: cynic::Id,
    pub op: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "ExecuteArgs"
)]
pub struct Execute {
    #[arguments(id: $id, op: $op)]
    pub execute: bool,
}

#[derive(cynic::QueryVariables)]
pub struct RegisterArgs {
    pub id: cynic::Id,
    pub register: U64,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Query",
    variables = "RegisterArgs"
)]
pub struct Register {
    #[arguments(id: $id, register: $register)]
    pub register: U64,
}

#[derive(cynic::QueryVariables)]
pub struct MemoryArgs {
    pub id: cynic::Id,
    pub start: U64,
    pub size: U64,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Query",
    variables = "MemoryArgs"
)]
pub struct Memory {
    #[arguments(id: $id, start: $start, size: $size)]
    pub memory: String,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SetBreakpointArgs {
    pub id: cynic::Id,
    pub bp: Breakpoint,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "SetBreakpointArgs"
)]
pub struct SetBreakpoint {
    #[arguments(id: $id, breakpoint: $bp)]
    pub set_breakpoint: bool,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct Breakpoint {
    pub contract: ContractId,
    pub pc: U64,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct SetSingleSteppingArgs {
    pub id: cynic::Id,
    pub enable: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "SetSingleSteppingArgs"
)]
pub struct SetSingleStepping {
    #[arguments(id: $id, enable: $enable)]
    pub set_single_stepping: bool,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct StartTxArgs {
    pub id: cynic::Id,
    pub tx: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "StartTxArgs"
)]
pub struct StartTx {
    #[arguments(id: $id, txJson: $tx)]
    pub start_tx: RunResult,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ContinueTxArgs {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema_path = "./assets/schema.sdl",
    graphql_type = "Mutation",
    variables = "ContinueTxArgs"
)]
pub struct ContinueTx {
    #[arguments(id: $id)]
    pub continue_tx: RunResult,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct RunResult {
    pub breakpoint: Option<OutputBreakpoint>,
    pub json_receipts: Vec<String>,
}

impl RunResult {
    pub fn receipts(&self) -> impl Iterator<Item = fuel_tx::Receipt> + '_ {
        self.json_receipts.iter().map(|r| {
            serde_json::from_str::<fuel_tx::Receipt>(r)
                .expect("Receipt deserialization failed, server/client version mismatch")
        })
    }
}

impl fmt::Display for RunResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let receipts: Vec<fuel_tx::Receipt> = self.receipts().collect();
        f.debug_struct("RunResult")
            .field("breakpoint", &self.breakpoint)
            .field("receipts", &receipts)
            .finish()
    }
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct OutputBreakpoint {
    pub contract: ContractId,
    pub pc: U64,
}

/// Generic graphql pagination query args
#[derive(cynic::QueryVariables, Debug, Default)]
pub struct ConnectionArgs {
    /// Skip until cursor (forward pagination)
    pub after: Option<String>,
    /// Skip until cursor (backward pagination)
    pub before: Option<String>,
    /// Retrieve the first n items in order (forward pagination)
    pub first: Option<i32>,
    /// Retrieve the last n items in order (backward pagination).
    /// Can't be used at the same time as `first`.
    pub last: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema_path = "./assets/schema.sdl")]
pub struct PageInfo {
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
}

/// Specifies the direction of a paginated query
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PageDirection {
    Forward,
    Backward,
}

/// Used to parameterize paginated queries
#[derive(Clone, Debug)]
pub struct PaginationRequest<T> {
    /// The cursor returned from a previous query to indicate an offset
    pub cursor: Option<T>,
    /// The number of results to take
    pub results: usize,
    /// The direction of the query (e.g. asc, desc order).
    pub direction: PageDirection,
}

impl<T: Into<String>> From<PaginationRequest<T>> for ConnectionArgs {
    fn from(req: PaginationRequest<T>) -> Self {
        match req.direction {
            PageDirection::Forward => Self {
                after: req.cursor.map(Into::into),
                before: None,
                first: Some(req.results as i32),
                last: None,
            },
            PageDirection::Backward => Self {
                after: None,
                before: req.cursor.map(Into::into),
                first: None,
                last: Some(req.results as i32),
            },
        }
    }
}

pub struct PaginatedResult<T, C> {
    pub cursor: Option<C>,
    pub results: Vec<T>,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConversionError {
    #[error("Field is required from the GraphQL response {0}")]
    MissingField(String),
    #[error("expected 0x prefix")]
    HexStringPrefixError,
    #[error("expected 32 bytes, received {0}")]
    HexString256LengthError(usize),
    #[error("hex parsing error {0}")]
    HexDecodingError(FromHexError),
    #[error("hex parsing error {0}")]
    HexError(String),
    #[error("failed integer conversion")]
    IntegerConversion,
    #[error("failed to deserialize transaction from bytes {0}")]
    TransactionFromBytesError(std::io::Error),
    #[error("failed to deserialize receipt from bytes {0}")]
    ReceiptFromBytesError(std::io::Error),
    #[error("failed to convert from bytes due to unexpected length")]
    BytesLength,
    #[error("Unknown variant of the {0} enum")]
    UnknownVariant(&'static str),
}

impl From<FromHexError> for ConversionError {
    fn from(hex_error: FromHexError) -> Self {
        Self::HexDecodingError(hex_error)
    }
}

impl From<ConversionError> for std::io::Error {
    fn from(e: ConversionError) -> Self {
        std::io::Error::new(ErrorKind::Other, e)
    }
}

impl From<TryFromIntError> for ConversionError {
    fn from(_: TryFromIntError) -> Self {
        ConversionError::IntegerConversion
    }
}

impl From<TryFromSliceError> for ConversionError {
    fn from(_: TryFromSliceError) -> Self {
        ConversionError::BytesLength
    }
}
