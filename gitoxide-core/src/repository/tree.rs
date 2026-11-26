use std::{
    borrow::Cow,
    io,
    io::{BufWriter, Write},
};

use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice, ByteVec},
    error::{ResultExt, bail, unsupported},
    object::bounded::Source,
    objs::find::Buffer,
    traverse::tree::source::Source as _,
};

use crate::OutputFormat;

mod entries {
    use std::collections::VecDeque;

    use gix::{
        bstr::{BStr, BString, ByteSlice, ByteVec},
        objs::tree::EntryRef,
        traverse::tree::visit::Action,
    };

    use crate::repository::tree::format_entry;

    #[cfg_attr(feature = "serde", derive(serde::Serialize))]
    #[derive(Default)]
    pub struct Statistics {
        pub num_trees: usize,
        pub num_links: usize,
        pub num_blobs: usize,
        pub num_blobs_exec: usize,
        pub num_submodules: usize,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        #[cfg_attr(not(feature = "serde"), allow(dead_code))]
        pub bytes: Option<u64>,
        #[cfg_attr(feature = "serde", serde(skip))]
        pub num_bytes: u64,
    }

    pub struct Traverse<'repo, 'a> {
        pub stats: Statistics,
        error: Option<gix::Error>,
        repo: Option<&'repo gix::Repository>,
        out: Option<&'a mut dyn std::io::Write>,
        path: BString,
        path_deque: VecDeque<BString>,
        listing: Option<&'a super::GitListing>,
    }

    impl<'repo, 'a> Traverse<'repo, 'a> {
        pub fn new(repo: Option<&'repo gix::Repository>, out: Option<&'a mut dyn std::io::Write>) -> Self {
            Traverse {
                stats: Default::default(),
                error: None,
                repo,
                out,
                path: BString::default(),
                path_deque: VecDeque::new(),
                listing: None,
            }
        }

        pub fn for_listing(out: &'a mut dyn std::io::Write, listing: &'a super::GitListing) -> Self {
            let mut delegate = Self::new(None, Some(out));
            delegate.listing = Some(listing);
            delegate
        }

        fn list_entry(&mut self, entry: &EntryRef<'_>) -> Action {
            use gix::error::ResultExt;
            let listing = self.listing.expect("called only for Git listings");
            if listing.includes(&self.path)
                && let Some(out) = &mut self.out
                && let Err(error) = listing.write_entry(out, entry, self.path.as_bstr()).or_error()
            {
                self.error = Some(error);
                return std::ops::ControlFlow::Break(());
            }
            std::ops::ControlFlow::Continue(false)
        }

        pub fn result(&mut self, result: gix::Result<()>) -> gix::Result<()> {
            self.error.take().map_or(result, Err)
        }

        fn check_interrupt(&mut self) -> bool {
            use gix::error::ResultExt;
            match gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED).or_error() {
                Ok(()) => true,
                Err(error) => {
                    self.error = Some(error);
                    false
                }
            }
        }

        fn pop_element(&mut self) {
            if let Some(pos) = self.path.rfind_byte(b'/') {
                self.path.resize(pos, 0);
            } else {
                self.path.clear();
            }
        }

        fn push_element(&mut self, name: &BStr) {
            if name.is_empty() {
                return;
            }
            if !self.path.is_empty() {
                self.path.push(b'/');
            }
            self.path.push_str(name);
        }
    }

    impl gix::traverse::tree::Visit for Traverse<'_, '_> {
        fn pop_back_tracked_path_and_set_current(&mut self) {
            self.path = self.path_deque.pop_back().unwrap_or_default();
        }

        fn pop_front_tracked_path_and_set_current(&mut self) {
            self.path = self.path_deque.pop_front().expect("every parent is set only once");
        }

        fn push_back_tracked_path_component(&mut self, component: &BStr) {
            self.push_element(component);
            self.path_deque.push_back(self.path.clone());
        }

        fn push_path_component(&mut self, component: &BStr) {
            self.push_element(component);
        }

        fn pop_path_component(&mut self) {
            self.pop_element();
        }

        fn visit_tree(&mut self, entry: &EntryRef<'_>) -> Action {
            if !self.check_interrupt() {
                return std::ops::ControlFlow::Break(());
            }
            if let Some(listing) = self.listing {
                if listing.descends_into(&self.path) {
                    return std::ops::ControlFlow::Continue(true);
                }
                return self.list_entry(entry);
            }
            self.stats.num_trees += 1;
            std::ops::ControlFlow::Continue(true)
        }

        fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> Action {
            use gix::error::ResultExt;
            if !self.check_interrupt() {
                return std::ops::ControlFlow::Break(());
            }
            if self.listing.is_some() {
                return self.list_entry(entry);
            }
            let size = match self.repo.map(|repo| repo.find_header(entry.oid)).transpose() {
                Ok(header) => header.map(|header| header.size()),
                // Gitlink commits need not be present in the superproject.
                Err(_) if entry.mode.is_commit() => None,
                Err(error) => {
                    self.error = Some(error);
                    return std::ops::ControlFlow::Break(());
                }
            };
            if let Some(out) = &mut self.out
                && let Err(error) = format_entry(out, entry, self.path.as_bstr(), size).or_error()
            {
                self.error = Some(error);
                return std::ops::ControlFlow::Break(());
            }
            if let Some(size) = size {
                self.stats.num_bytes += size;
            }

            use gix::object::tree::EntryKind::*;
            match entry.mode.kind() {
                Commit => self.stats.num_submodules += 1,
                Blob => self.stats.num_blobs += 1,
                BlobExecutable => self.stats.num_blobs_exec += 1,
                Link => self.stats.num_links += 1,
                Tree => unreachable!("BUG"),
            }
            std::ops::ControlFlow::Continue(true)
        }
    }
}

#[cfg_attr(not(feature = "serde"), allow(unused_variables))]
pub fn info(
    repo: gix::Repository,
    treeish: Option<&str>,
    extended: bool,
    format: OutputFormat,
    out: impl io::Write,
    mut err: impl io::Write,
) -> Result<()> {
    // Resolve the revision before the note, so a bad revision keeps its own failure on a closed stderr.
    let tree_id = treeish_to_tree(treeish, &repo)?;
    if format == OutputFormat::Human {
        writeln!(err, "Only JSON is implemented - using that instead").or_error()?;
    }
    let source = Source::new(
        &repo,
        gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
    )?;
    let mut buffer = Buffer::default();

    let mut delegate = entries::Traverse::new(extended.then_some(&repo), None);
    let result = gix::traverse::tree::breadthfirst(
        source.find_tree(&tree_id, &mut buffer)?,
        gix::traverse::tree::breadthfirst::State::default(),
        source.clone(),
        &mut delegate,
    );
    delegate.result(result)?;

    #[cfg(feature = "serde")]
    {
        delegate.stats.bytes = extended.then_some(delegate.stats.num_bytes);
        serde_json::to_writer_pretty(out, &delegate.stats).or_error()?;
    }

    Ok(())
}

pub fn entries(
    repo: gix::Repository,
    treeish: Option<&str>,
    recursive: bool,
    extended: bool,
    format: OutputFormat,
    out: impl io::Write,
) -> Result<()> {
    if format != OutputFormat::Human {
        bail!(unsupported("Only human output format is supported at the moment"));
    }

    let tree_id = treeish_to_tree(treeish, &repo)?;
    let source = Source::new(
        &repo,
        gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
    )?;

    let mut write = BufWriter::new(out);
    if recursive {
        let mut delegate = entries::Traverse::new(extended.then_some(&repo), Some(&mut write));
        let result = gix::traverse::tree::depthfirst(
            tree_id,
            gix::traverse::tree::depthfirst::State::default(),
            source,
            &mut delegate,
        );
        delegate.result(result)?;
    } else {
        let mut buffer = Buffer::default();
        for entry in source.find_tree(&tree_id, &mut buffer)? {
            gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED).or_error()?;
            let entry = entry?;
            format_entry(
                &mut write,
                &entry,
                entry.filename,
                extended
                    .then(|| repo.find_header(entry.oid).map(|o| o.size()))
                    .transpose()?,
            )
            .or_error()?;
        }
    }
    write.flush().or_error()?;

    Ok(())
}

fn treeish_to_tree(treeish: Option<&str>, repo: &gix::Repository) -> Result<gix::ObjectId> {
    let spec = treeish.map_or_else(|| "@^{tree}".into(), |spec| format!("{spec}^{{tree}}"));
    Ok(repo.rev_parse_single(spec.as_str())?.detach())
}

/// Output options for Git's tree listing.
#[derive(Default)]
pub struct LsTreeOptions {
    pub recursive: bool,
    pub name_only: bool,
    pub nul: bool,
}

/// List a revision's tree in Git's format, with literal repository paths as filters.
pub fn ls_tree(
    repo: &gix::Repository,
    treeish: &BStr,
    paths: &[BString],
    options: LsTreeOptions,
    out: impl Write,
    mut err: impl Write,
) -> Result<()> {
    use super::git_spellings::exit;

    let Ok(id) = repo.rev_parse_single(treeish) else {
        if gix::ObjectId::from_hex(treeish).is_ok_and(|id| id.kind() == repo.object_hash()) {
            writeln!(err, "fatal: not a tree object").ok();
        } else {
            writeln!(err, "fatal: Not a valid object name {treeish}").ok();
        }
        return Err(exit(128));
    };
    let source = Source::deferred(repo);
    let Ok(tree_id) = source.peel(id.detach(), gix::objs::Kind::Tree) else {
        writeln!(err, "fatal: not a tree object").ok();
        return Err(exit(128));
    };
    let prefix_path = repo.prefix()?.unwrap_or_else(|| std::path::Path::new(""));
    let prefix = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(prefix_path)?).into_owned();
    let mut selected = listing_paths(repo, prefix_path, paths, err)?;
    if paths.is_empty() && !prefix.is_empty() {
        let mut selected_prefix = prefix.clone();
        selected_prefix.push(b'/');
        selected.push(selected_prefix);
    }
    GitListing {
        options,
        selected,
        prefix,
        quote_high: super::diff::quote_path(repo)?,
    }
    .write(source, tree_id, out)
}

fn listing_paths(
    repo: &gix::Repository,
    prefix: &std::path::Path,
    paths: &[BString],
    mut err: impl Write,
) -> Result<Vec<BString>> {
    use gix::pathspec::{MagicSignature, SearchMode};

    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let defaults = repo.pathspec_defaults()?;
    let root = gix::path::realpath_opts(
        repo.workdir().unwrap_or_else(|| repo.git_dir()),
        repo.current_dir(),
        gix::path::realpath::MAX_SYMLINKS,
    )?;
    let mut selected = Vec::with_capacity(paths.len());
    for path in paths {
        if path.is_empty() {
            writeln!(
                err,
                "fatal: empty string is not a valid pathspec. please use . instead if you meant to match all paths"
            )
            .ok();
            return Err(super::git_spellings::exit(128));
        }
        let mut pattern = gix::pathspec::parse(path, defaults)?;
        let unsupported = [
            (pattern.search_mode == SearchMode::PathAwareGlob, "'glob'"),
            (pattern.signature.contains(MagicSignature::ICASE), "'icase'"),
            (pattern.is_excluded(), "'exclude' (mnemonic: '!')"),
            (!pattern.attributes.is_empty(), "'attr'"),
        ]
        .into_iter()
        .filter_map(|(unsupported, name)| unsupported.then_some(name))
        .collect::<Vec<_>>();
        if !unsupported.is_empty() {
            writeln!(
                err,
                "fatal: {path}: pathspec magic not supported by this command: {}",
                unsupported.join(", ")
            )
            .ok();
            return Err(super::git_spellings::exit(128));
        }
        let descend = pattern.signature.contains(MagicSignature::MUST_BE_DIR) || pattern.path() == b".";
        pattern.normalize(prefix, &root)?;
        let mut normalized = if pattern.always_matches() {
            BString::default()
        } else {
            pattern.path().to_owned()
        };
        if descend && !normalized.is_empty() {
            normalized.push(b'/');
        }
        selected.push(normalized);
    }
    Ok(selected)
}

pub(super) fn pretty_tree(repo: &gix::Repository, tree_id: gix::ObjectId, out: impl Write) -> Result<()> {
    GitListing {
        options: LsTreeOptions::default(),
        selected: Vec::new(),
        prefix: BString::default(),
        quote_high: super::diff::quote_path(repo)?,
    }
    .write(Source::deferred(repo), tree_id, out)
}

struct GitListing {
    options: LsTreeOptions,
    selected: Vec<BString>,
    prefix: BString,
    quote_high: bool,
}

impl GitListing {
    fn includes(&self, path: &[u8]) -> bool {
        self.selected.is_empty()
            || self.selected.iter().any(|selected| {
                selected.is_empty()
                    || path == selected.as_slice()
                    || if selected.ends_with(b"/") {
                        path.starts_with(selected)
                    } else {
                        is_below(path, selected)
                    }
            })
    }

    fn descends_into(&self, path: &[u8]) -> bool {
        (self.options.recursive && self.includes(path)) || self.selected.iter().any(|selected| is_below(selected, path))
    }

    fn write_entry(&self, mut out: impl Write, entry: &gix::objs::tree::EntryRef<'_>, path: &BStr) -> io::Result<()> {
        if !self.options.name_only {
            let kind = entry.mode.kind();
            let object_kind = match kind {
                gix::objs::tree::EntryKind::Tree => "tree",
                gix::objs::tree::EntryKind::Commit => "commit",
                _ => "blob",
            };
            write!(out, "{:06o} {object_kind} {}\t", kind as u16, entry.oid)?;
        }
        let path = relative_listing_path(path, self.prefix.as_bstr());
        if self.options.nul {
            out.write_all(&path)?;
            out.write_all(b"\0")
        } else {
            out.write_all(&super::diff::quote_patch_path(&path, self.quote_high))?;
            out.write_all(b"\n")
        }
    }

    fn write(&self, source: Source<'_>, tree_id: gix::ObjectId, out: impl Write) -> Result<()> {
        let mut out = BufWriter::new(out);
        let mut delegate = entries::Traverse::for_listing(&mut out, self);
        let result = gix::traverse::tree::depthfirst(
            tree_id,
            gix::traverse::tree::depthfirst::State::default(),
            source,
            &mut delegate,
        );
        delegate.result(result)?;
        out.flush().or_error()?;
        Ok(())
    }
}

fn is_below(path: &[u8], directory: &[u8]) -> bool {
    path.strip_prefix(directory).is_some_and(|rest| rest.starts_with(b"/"))
}

fn relative_listing_path<'a>(path: &'a BStr, prefix: &BStr) -> Cow<'a, BStr> {
    if prefix.is_empty() {
        return Cow::Borrowed(path);
    }
    if path == prefix {
        return Cow::Borrowed(b"./".as_bstr());
    }
    let mut rest = path.as_bytes();
    let mut base = prefix.split(|byte| *byte == b'/').peekable();
    while let Some(component) = base.peek() {
        let Some(suffix) = rest.strip_prefix(*component) else {
            break;
        };
        if let Some(suffix) = suffix.strip_prefix(b"/") {
            rest = suffix;
        } else if suffix.is_empty() {
            rest = suffix;
        } else {
            break;
        }
        base.next();
    }
    let mut steps = BString::default();
    for _ in base {
        steps.push_str(b"../");
    }
    if steps.is_empty() {
        Cow::Borrowed(rest.as_bstr())
    } else {
        steps.push_str(rest);
        Cow::Owned(steps)
    }
}

fn format_entry(
    mut out: impl io::Write,
    entry: &gix::objs::tree::EntryRef<'_>,
    filename: &gix::bstr::BStr,
    size: Option<u64>,
) -> std::io::Result<()> {
    use gix::objs::tree::EntryKind::*;
    write!(
        out,
        "{} {}{} ",
        match entry.mode.kind() {
            Tree => "TREE",
            Blob => "BLOB",
            BlobExecutable => " EXE",
            Link => "LINK",
            Commit => "SUBM",
        },
        entry.oid,
        size.map_or_else(|| "".into(), |s| Cow::Owned(format!(" {s}")))
    )?;
    out.write_all(filename)?;
    out.write_all(b"\n")
}
