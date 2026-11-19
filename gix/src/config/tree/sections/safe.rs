use crate::{
    config,
    config::tree::{Key, Safe, Section, keys},
};

impl Safe {
    /// The `safe.directory` key
    pub const DIRECTORY: keys::Any = keys::Any::new("directory", &config::Tree::SAFE);
}

impl Safe {
    /// Implements the directory filter to trust only global and system files, for use with `safe.directory`.
    pub fn directory_filter(meta: &gix_config::file::Metadata) -> bool {
        let kind = meta.source.kind();
        kind == gix_config::source::Kind::System || kind == gix_config::source::Kind::Global
    }

    /// The sources from which `safe.directory` may *grant* ownership trust to a repository: git's
    /// "protected configuration", which is the system and global files plus the command scope.
    ///
    /// The command scope is configuration supplied by the invoker of the process: `-c` overrides
    /// ([`Source::Cli`](gix_config::Source::Cli)) and the `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>`
    /// environment ([`Source::Env`](gix_config::Source::Env)). Repository-local configuration, programmatic
    /// [`Source::Api`](gix_config::Source::Api) overrides and environment variable overrides of specific keys are
    /// never consulted.
    ///
    /// Use [`Self::directory_filter()`] to decide which configuration *sections* are trusted by default;
    /// this filter only widens the ownership decision.
    pub fn protected_directory_filter(meta: &gix_config::file::Metadata) -> bool {
        Self::directory_filter(meta) || matches!(meta.source, gix_config::Source::Cli | gix_config::Source::Env)
    }
}

impl Section for Safe {
    fn name(&self) -> &str {
        "safe"
    }

    fn keys(&self) -> &[&dyn Key] {
        &[&Self::DIRECTORY]
    }
}
