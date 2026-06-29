//! Covenant engine — unconditional layer that knows the trailer format and
//! dispatch contract but not any specific schema vocabulary.

mod app_schema;
pub mod cite;
pub mod cid;
pub mod content_node;
mod error;
pub mod frontmatter;
pub mod interface_ref;
pub mod object_store;
pub mod signing;
mod trailer_block;
mod trailer_set;

pub use app_schema::AppSchema;
pub use cite::{extract_cites, extract_id, SlugIndex};
pub use cid::{BritCid, CidParseError};
pub use content_node::{CborError, ContentNode};
pub use error::{EngineError, ValidationError};
pub use frontmatter::{canonical_body, drift_fingerprint, split_frontmatter};
pub use interface_ref::{EdgeKind, EdgeRole, InterfaceRef, parse_cite_line};
pub use object_store::{LocalObjectStore, ObjectStoreError};
pub use signing::{verify_signature, verify_signed_node, AgentKey, AgentKeyError, Signed};
pub use trailer_block::parse_trailer_block;
pub use trailer_set::TrailerSet;
pub mod verdict;
pub use verdict::{verdict, Verdict};
