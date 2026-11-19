use std::path::PathBuf;

use super::Options;
use crate::{Result, ThreadSafeRepository, bstr::BString, config, open::Permissions};

impl Default for Options {
    fn default() -> Self {
        Options {
            object_store_slots: None,
            permissions: Default::default(),
            git_dir_trust: None,
            filter_config_section: None,
            lossy_config: false,
            lenient_config: true,
            bail_if_untrusted: false,
            open_path_as_is: false,
            api_config_overrides: Vec::new(),
            cli_config_overrides: Vec::new(),
            git_installation_config_path: None,
            system_config_path: None,
            use_repository_local_environment: true,
            current_dir: None,
        }
    }
}

/// Instantiation
impl Options {
    /// Options configured to prevent accessing anything else than the repository configuration file, prohibiting
    /// accessing the environment or spreading beyond the git repository location.
    pub fn isolated() -> Self {
        Options::default().permissions(Permissions::isolated())
    }
}

/// Generic modification
impl Options {
    /// An adapter to allow calling any builder method on this instance despite only having a mutable reference.
    pub fn modify(&mut self, f: impl FnOnce(Self) -> Self) {
        *self = f(std::mem::take(self));
    }
}

/// Builder methods
impl Options {
    /// Apply the given configuration `values` like `init.defaultBranch=special` or `core.bool-implicit-true` in memory to as early
    /// as the configuration is initialized to allow affecting the repository instantiation phase, both on disk or when opening.
    /// The configuration is marked with [source API][gix_config::Source::Api].
    pub fn config_overrides(mut self, values: impl IntoIterator<Item = impl Into<BString>>) -> Self {
        self.api_config_overrides = values.into_iter().map(Into::into).collect();
        self
    }

    /// Set configuration values of the form `core.abbrev=5` or `remote.origin.url = foo` or `core.bool-implicit-true` for application
    /// as CLI overrides to the repository configuration, marked with [source CLI][gix_config::Source::Cli].
    /// These are equivalent to CLI overrides passed with `-c` in `git`, for example.
    pub fn cli_overrides(mut self, values: impl IntoIterator<Item = impl Into<BString>>) -> Self {
        self.cli_config_overrides = values.into_iter().map(Into::into).collect();
        self
    }

    /// Use `path` as Git's installation configuration file instead of discovering it.
    ///
    /// Common locations are `$(prefix)/etc/gitconfig` on Unix,
    /// `/Applications/Xcode.app/Contents/Developer/usr/share/git-core/gitconfig` for Apple Git,
    /// and `C:/Program Files/Git/etc/gitconfig` for Git for Windows.
    ///
    /// The file is loaded only when [`permissions.config.git_binary`][Permissions::config] is enabled.
    pub fn git_installation_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.git_installation_config_path = Some(path.into().into_boxed_path());
        self
    }

    /// Use `path` as the system configuration file instead of discovering it.
    ///
    /// Common locations are `/etc/gitconfig` on Unix and `C:/ProgramData/Git/config` on Windows.
    ///
    /// The file is loaded only when [`permissions.config.system`][Permissions::config] is enabled.
    pub fn system_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.system_config_path = Some(path.into().into_boxed_path());
        self
    }

    /// Set the amount of slots to use for the object database. It's a value that doesn't need changes on the client, typically,
    /// but should be controlled on the server.
    ///
    /// If unset, the slots follow the repository's final trust: sized from disk when fully trusted,
    /// and capped when trust remains reduced. A value set here is used regardless of trust.
    pub fn object_store_slots(mut self, slots: gix_odb::store::init::Slots) -> Self {
        self.object_store_slots = Some(slots);
        self
    }

    // TODO: tests
    /// Set the given permissions, which are typically derived by a `Trust` level.
    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = permissions;
        self
    }

    /// If `true`, default `false`, we will not modify the incoming path for this open call to assure it is a `.git` directory.
    ///
    /// If `true`, we will try to open the input directory as is, even though it doesn't appear to be a `git` repository
    /// due to the lack of `.git` suffix or because its basename is not `.git` as in `worktree/.git`.
    /// This option is consumed while resolving the path and isn't retained in the opened repository's options.
    pub fn open_path_as_is(mut self, enable: bool) -> Self {
        self.open_path_as_is = enable;
        self
    }

    /// Set the trust level of the `.git` directory we are about to open.
    ///
    /// This can be set manually to force trust even though otherwise it might
    /// not be fully trusted, leading to limitations in how configuration files
    /// are interpreted.
    ///
    /// If not called explicitly, trust is determined via [`gix_sec::Trust::from_path_ownership()`] for the
    /// candidate path before resolving a possible gitfile, then lowered as needed by ownership checks of the
    /// resolved git directory, common directory, and existing worktree. Setting trust explicitly bypasses
    /// these initial ownership checks when opening, but not when discovering a repository.
    ///
    /// # Security Warning
    ///
    /// Use with extreme care and only if it's absolutely known that the repository
    /// is always controlled by the desired user. This only bypasses the ownership checks described above
    /// when the [`open()`][Self::open()] method is used, as opposed to discovery. Worktree ownership and
    /// `safe.directory` are still considered later during opening.
    pub fn with(mut self, trust: gix_sec::Trust) -> Self {
        self.git_dir_trust = trust.into();
        self
    }

    /// If true, default false, and if the repository's trust level is not `Full`
    /// (see [`with()`][Self::with()] for more), then the open operation will fail.
    ///
    /// Use this to mimic `git`s way of handling untrusted repositories. Note that `gitoxide` solves
    /// this by not using configuration from untrusted sources and by generally being secured against
    /// doctored input files which at worst could cause out-of-memory at the time of writing.
    pub fn bail_if_untrusted(mut self, toggle: bool) -> Self {
        self.bail_if_untrusted = toggle;
        self
    }

    /// Set the filter which determines if a configuration section can be used to read values from,
    /// hence it returns true if it is eligible.
    ///
    /// The default filter selects sections whose trust level is [`full`][gix_sec::Trust::Full] or
    /// whose source is not [`repository-local`][gix_config::source::Kind::Repository].
    pub fn filter_config_section(mut self, filter: fn(&gix_config::file::Metadata) -> bool) -> Self {
        self.filter_config_section = Some(filter);
        self
    }

    /// If set, default is false, configuration will be read without retaining non-essential information like comments
    /// or whitespace to optimize lookup performance.
    ///
    /// This will prevent displaying or editing configuration losslessly.
    pub fn lossy_config(mut self, toggle: bool) -> Self {
        self.lossy_config = toggle;
        self
    }

    /// If set, default is false, invalid configuration values will cause an error even if these can safely be defaulted.
    ///
    /// This is recommended for all applications that prefer correctness over usability.
    /// `git` itself defaults to strict configuration mode, flagging incorrect configuration immediately.
    ///
    /// Failure to read configuration files due to IO errors will also be a hard error if this mode is enabled, otherwise
    /// these errors will merely be logged.
    pub fn strict_config(mut self, toggle: bool) -> Self {
        self.lenient_config = !toggle;
        self
    }

    /// Open a repository at `path` with the options set so far.
    pub fn open(self, path: impl Into<PathBuf>) -> Result<ThreadSafeRepository> {
        ThreadSafeRepository::open_opts(path, self)
    }
}

impl Options {
    /// Prevent repository-local environment overrides from being inherited when opening another repository.
    ///
    /// To be used when opening a nested repository, like worktrees or submodules.
    pub(crate) fn without_repository_environment_overrides(mut self) -> Self {
        self.use_repository_local_environment = false;
        self
    }

    pub(crate) fn current_dir_or_empty(&self) -> &std::path::Path {
        self.current_dir.as_deref().unwrap_or(std::path::Path::new(""))
    }

    /// Return the object-store slots for a repository whose trust is final, after any `safe.directory` promotion.
    ///
    /// Slots set explicitly via [`Self::object_store_slots()`] always win. Otherwise fully trusted repositories size
    /// their slots from disk. A repository that remains at reduced trust is sized from the pack indexes present in
    /// `objects_dir` (anchored at `current_dir`) and its alternates, plus headroom for packs that arrive while it is
    /// open, and is refused when it holds more than [`REDUCED_TRUST_MAX_PACK_INDEXES`].
    pub(crate) fn resolve_object_store_slots(
        slots: Option<gix_odb::store::init::Slots>,
        trust: gix_sec::Trust,
        objects_dir: &std::path::Path,
        current_dir: &std::path::Path,
    ) -> crate::Result<gix_odb::store::init::Slots> {
        if let Some(slots) = slots {
            return Ok(slots);
        }
        Ok(match trust {
            gix_sec::Trust::Full => Default::default(),
            gix_sec::Trust::Reduced => {
                gix_odb::store::init::Slots::Given(reduced_trust_slots(objects_dir, current_dir)?)
            }
        })
    }
}

/// The most pack indexes a repository at reduced trust may hold and still be opened.
///
/// Reduced trust bounds what a repository owned by someone else can make this process do: every index is listed at
/// open, memory-mapped when first searched, and a lookup of a missing object searches all of them. A bound in the
/// hundreds keeps that work, and the address-space and map counts it costs, small and fixed, while sitting an order of
/// magnitude above repositories git maintains itself: `gc.autoPackLimit` consolidates past 50 packs. Repositories
/// whose packs are never consolidated (one more per fetch) reach it only after hundreds of fetches.
pub const REDUCED_TRUST_MAX_PACK_INDEXES: usize = 512;

/// Slots kept even for a repository with few packs, as before slots were sized at reduced trust.
const REDUCED_TRUST_MIN_SLOTS: usize = 32;

/// Slots for a reduced-trust repository with the pack indexes now in `objects_dir`: those indexes plus half as many
/// again (at least 16) for packs written while it is open, at least [`REDUCED_TRUST_MIN_SLOTS`], and never more than
/// [`REDUCED_TRUST_MAX_PACK_INDEXES`]. More indexes than that is an error naming both cures.
fn reduced_trust_slots(objects_dir: &std::path::Path, current_dir: &std::path::Path) -> crate::Result<u16> {
    use gix_error::ResultExt;

    let objects_dir = current_dir.join(objects_dir);
    let mut db_paths = gix_odb::alternate::resolve(objects_dir.clone(), current_dir)
        .or_raise(|| gix_error::message("Could not resolve object database alternates"))?;
    db_paths.insert(0, objects_dir);
    let mut indexes = 0_usize;
    'count: for db_path in db_paths {
        let entries = match std::fs::read_dir(db_path.join("pack")) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err).or_raise(|| gix_error::message("Could not read pack directory"));
            }
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            // What `Slots::AsNeededByDiskState` counts: an index with its pack.
            if path.extension() == Some(std::ffi::OsStr::new("idx")) && path.with_extension("pack").is_file() {
                indexes += 1;
                if indexes > REDUCED_TRUST_MAX_PACK_INDEXES {
                    // Stop listing: the work done for a repository we refuse stays bounded too.
                    break 'count;
                }
            }
        }
    }
    if indexes > REDUCED_TRUST_MAX_PACK_INDEXES {
        return Err(too_many_pack_indexes(REDUCED_TRUST_MAX_PACK_INDEXES, indexes));
    }
    let slots = (indexes + (indexes / 2).max(16)).clamp(REDUCED_TRUST_MIN_SLOTS, REDUCED_TRUST_MAX_PACK_INDEXES);
    Ok(u16::try_from(slots).expect("bounded by REDUCED_TRUST_MAX_PACK_INDEXES"))
}

/// The refusal of a reduced-trust repository whose pack indexes exceed `limit`, with the slot-capacity metadata
/// `current` (slots available) and `needed` (additional slots) that `gix-odb` reports when it runs out of slots.
fn too_many_pack_indexes(limit: usize, at_least: usize) -> crate::Error {
    crate::Error::from_error(
        gix_error::Message::new(format!(
            "The repository has more than {limit} pack indexes, the most opened for a checkout the current user does \
             not own. Consolidate its packs (`git repack -a -d`) or grant trust for this checkout with \
             `safe.directory` (a global entry, `-c safe.directory=<path>` or GIT_CONFIG_COUNT)"
        ))
        .with("current", limit)
        .with("needed", at_least - limit),
    )
}

impl gix_sec::trust::DefaultForLevel for Options {
    fn default_for_level(level: gix_sec::Trust) -> Self {
        match level {
            gix_sec::Trust::Full => Options {
                object_store_slots: None,
                permissions: Permissions::default_for_level(level),
                git_dir_trust: gix_sec::Trust::Full.into(),
                filter_config_section: Some(config::section::is_trusted),
                lossy_config: false,
                bail_if_untrusted: false,
                lenient_config: true,
                open_path_as_is: false,
                api_config_overrides: Vec::new(),
                cli_config_overrides: Vec::new(),
                git_installation_config_path: None,
                system_config_path: None,
                use_repository_local_environment: true,
                current_dir: None,
            },
            gix_sec::Trust::Reduced => Options {
                object_store_slots: None,
                permissions: Permissions::default_for_level(level),
                git_dir_trust: gix_sec::Trust::Reduced.into(),
                filter_config_section: Some(config::section::is_trusted),
                bail_if_untrusted: false,
                lenient_config: true,
                open_path_as_is: false,
                lossy_config: false,
                api_config_overrides: Vec::new(),
                cli_config_overrides: Vec::new(),
                git_installation_config_path: None,
                system_config_path: None,
                use_repository_local_environment: true,
                current_dir: None,
            },
        }
    }
}
