pub mod cloud;
pub mod crypto;
pub mod error;
pub mod ingestion;
pub mod parser;
pub mod sink;
pub mod telemetry;
pub mod transform;

pub type EngineResult<T> = Result<T, error::MainframeError>;
