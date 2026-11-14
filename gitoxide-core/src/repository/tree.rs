use std::{
    borrow::Cow,
    io,
    io::{BufWriter, Write},
};

use gix::{
    Result,
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
            }
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

        fn visit_tree(&mut self, _entry: &EntryRef<'_>) -> Action {
            if !self.check_interrupt() {
                return std::ops::ControlFlow::Break(());
            }
            self.stats.num_trees += 1;
            std::ops::ControlFlow::Continue(true)
        }

        fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> Action {
            use gix::error::ResultExt;
            if !self.check_interrupt() {
                return std::ops::ControlFlow::Break(());
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
    if format == OutputFormat::Human {
        writeln!(err, "Only JSON is implemented - using that instead").or_error()?;
    }

    let tree_id = treeish_to_tree(treeish, &repo)?;
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
