//! Covenant engine — unconditional layer that knows the trailer format and
//! dispatch contract but not any specific schema vocabulary.

mod app_schema;
mod error;
mod trailer_block;
mod trailer_set;

pub use app_schema::AppSchema;
pub use error::{EngineError, ValidationError};
pub use trailer_block::parse_trailer_block;
pub use trailer_set::TrailerSet;
