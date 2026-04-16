//! Covenant engine — unconditional layer that knows the trailer format and
//! dispatch contract but not any specific schema vocabulary.

mod app_schema;
pub mod cid;
pub mod content_node;
mod error;
pub mod object_store;
pub mod signing;
mod trailer_block;
mod trailer_set;

pub use app_schema::AppSchema;
pub use cid::{BritCid, CidParseError};
pub use content_node::ContentNode;
pub use error::{EngineError, ValidationError};
pub use object_store::{LocalObjectStore, ObjectStoreError};
pub use signing::{verify_signature, verify_signed_node, AgentKey, AgentKeyError, Signed};
pub use trailer_block::parse_trailer_block;
pub use trailer_set::TrailerSet;
