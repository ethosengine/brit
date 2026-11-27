use std::{fmt::Formatter, path::PathBuf};

use gix::{
    Result,
    bstr::BString,
    error::{ResultExt, message},
};

#[cfg(feature = "archive")]
pub mod archive;
pub mod branch;
pub mod cat;
pub use cat::function::cat;
pub mod authoring;
pub mod blame;
pub mod commit;
pub mod config;
mod credential;
pub use credential::function as credential;
pub mod attributes;
#[cfg(feature = "clean")]
pub mod clean;
#[cfg(feature = "clean")]
pub use clean::function::clean;
pub mod diff;
pub mod dirty;
pub mod dirwalk;
mod editor;
pub mod feedback;
pub mod git_spellings;
pub use editor::function as editor;
#[cfg(feature = "blocking-client")]
pub mod clone;
pub mod exclude;
#[cfg(feature = "blocking-client")]
pub mod fetch;
#[cfg(feature = "blocking-client")]
mod notes_refspec;
#[cfg(feature = "blocking-client")]
pub mod push;
#[cfg(feature = "blocking-client")]
pub use clone::function::clone;
#[cfg(feature = "blocking-client")]
pub use fetch::function::fetch;

pub mod commitgraph;
mod fsck;
pub use fsck::function as fsck;
pub mod grep;
pub mod index;
pub mod log;
pub mod mailmap;
pub mod maintenance;
mod merge_base;
pub use merge_base::{is_ancestor as merge_base_is_ancestor, merge_base};
pub mod fsmonitor;
pub mod merge;
pub mod odb;
mod pickaxe;
#[cfg(feature = "blocking-client")]
pub mod pull;
pub mod remote;
pub mod replay;
pub mod revision;
pub mod show;
pub mod status;
pub mod submodule;
#[cfg(feature = "blocking-client")]
mod submodule_update;
pub mod switch;
pub mod tag;
pub mod tree;
pub mod verify;
pub mod watch;
pub mod worktree;

pub fn init(directory: Option<PathBuf>) -> Result<gix::discover::repository::Path> {
    gix::create::into(
        directory.unwrap_or_default(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
    )
    .or_raise(|| message("Repository initialization failed"))
}

pub enum PathsOrPatterns {
    Paths(Box<dyn std::iter::Iterator<Item = std::io::Result<BString>>>),
    Patterns(Vec<BString>),
}

struct HexId<'a>(gix::Id<'a>, bool);

impl<'a> HexId<'a> {
    pub fn new(id: gix::Id<'a>, long_hex: bool) -> Self {
        HexId(id, long_hex)
    }
}

impl std::fmt::Display for HexId<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let HexId(id, long_hex) = self;
        if *long_hex {
            id.fmt(f)
        } else {
            id.shorten_or_id().fmt(f)
        }
    }
}
