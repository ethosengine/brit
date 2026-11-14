pub(crate) mod function {
    use crate::OutputFormat;
    use crate::repository::HexId;
    use gix::odb::store::RefreshMode;
    use gix::revision::plumbing::Spec;
    use gix::{
        Result,
        error::{ResultExt, bail, message},
        prelude::ObjectIdExt,
        revision::walk::Sorting,
    };
    use std::{borrow::Cow, ffi::OsString};

    pub fn list(
        mut repo: gix::Repository,
        spec: OsString,
        mut out: impl std::io::Write,
        long_hashes: bool,
        format: OutputFormat,
    ) -> Result<()> {
        if format != OutputFormat::Human {
            bail!(gix::error::unsupported("Only human output is currently supported"));
        }
        let graph = repo
            .commit_graph_if_enabled()
            .or_raise(|| message("a commitgraph is required, but none was found"))?;
        repo.object_cache_size_if_unset(4 * 1024 * 1024);
        repo.objects.refresh = RefreshMode::Never;

        let spec = gix::path::os_str_into_bstr(&spec)?;
        let spec = repo.rev_parse(spec)?.detach();
        let commits = match spec {
            Spec::Include(id) => connected_commit_id(&repo, id)?
                .ancestors()
                .sorting(Sorting::ByCommitTime(Default::default()))
                .all()?,
            Spec::Range { from, to } => connected_commit_id(&repo, to)?
                .ancestors()
                .sorting(Sorting::ByCommitTime(Default::default()))
                .with_hidden(Some(connected_commit_id(&repo, from)?))
                .all()?,
            Spec::Exclude(_) | Spec::Merge { .. } | Spec::IncludeOnlyParents(_) | Spec::ExcludeParents(_) => {
                bail!("The spec isn't currently supported: {spec:?}".unsupported())
            }
        };
        for commit in commits {
            gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED).or_error()?;
            let commit = commit?;
            writeln!(
                out,
                "{} {} {} {}",
                HexId::new(commit.id(), long_hashes),
                commit.commit_time.expect("traversal with date"),
                commit.parent_ids.len(),
                graph
                    .as_ref()
                    .map_or(Cow::Borrowed(""), |graph| graph.commit_by_id(commit.id).map_or_else(
                        || Cow::Borrowed("<NOT IN GRAPH-CACHE>"),
                        |c| Cow::Owned(format!(
                            "{} {}",
                            HexId::new(c.root_tree_id().to_owned().attach(&repo), long_hashes),
                            c.generation()
                        ))
                    ))
            )
            .or_error()?;
        }
        Ok(())
    }

    fn connected_commit_id(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::Id<'_>> {
        Ok(gix::object::bounded::Source::deferred(repo)
            .peel(id, gix::object::Kind::Commit)
            .or_raise(|| message("Need committish as starting point"))?
            .attach(repo))
    }
}

#[cfg(test)]
mod tests {
    use gix::error::ResultExt;

    #[test]
    fn cancellation_and_output_errors() -> gix_testtools::TestResult {
        if gix_testtools::run_in_isolated_process()? {
            return Ok(());
        }
        let fixture = gix_testtools::tempfile::tempdir().or_error()?;
        for args in [
            vec!["init", "-q", "--initial-branch=main"],
            vec!["commit", "--allow-empty", "-qm", "initial"],
            vec!["commit-graph", "write", "--reachable"],
        ] {
            assert!(
                gix_testtools::git_command(fixture.path())
                    .args(args)
                    .status()
                    .or_error()?
                    .success()
            );
        }
        let repo = gix::open_opts(fixture.path(), gix::open::Options::isolated())?;
        gix::interrupt::IS_INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
        let start = std::time::Instant::now();
        let result = super::function::list(
            repo.clone(),
            "HEAD".into(),
            std::io::sink(),
            true,
            crate::OutputFormat::Human,
        );
        let elapsed = start.elapsed();
        gix::interrupt::IS_INTERRUPTED.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(result.expect_err("cancelled listing").is_cancelled());
        assert!(elapsed < std::time::Duration::from_secs(1));
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let error = super::function::list(repo.clone(), "HEAD".into(), Broken, true, crate::OutputFormat::Human)
            .expect_err("output failure");
        assert_eq!(
            error.downcast_any_ref::<std::io::Error>().expect("IO cause").kind(),
            std::io::ErrorKind::BrokenPipe
        );
        for free in [false, true] {
            gix::interrupt::IS_INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
            let start = std::time::Instant::now();
            let result = verify(&repo, std::io::sink(), free);
            let elapsed = start.elapsed();
            gix::interrupt::IS_INTERRUPTED.store(false, std::sync::atomic::Ordering::SeqCst);
            assert!(result.expect_err("cancelled verification").is_cancelled());
            assert!(elapsed < std::time::Duration::from_secs(1));
            let error = verify(&repo, Broken, free).expect_err("statistics output failure");
            assert_eq!(
                error.downcast_any_ref::<std::io::Error>().expect("IO cause").kind(),
                std::io::ErrorKind::BrokenPipe
            );
        }
        Ok(())
    }

    fn verify(repo: &gix::Repository, out: impl std::io::Write, free: bool) -> gix::Result<()> {
        if free {
            crate::commitgraph::verify(
                repo.git_dir().join("objects/info"),
                crate::commitgraph::verify::Context {
                    err: std::io::sink(),
                    out,
                    output_statistics: Some(crate::OutputFormat::Human),
                },
            )?;
        } else {
            crate::repository::commitgraph::verify(
                repo.clone(),
                crate::repository::commitgraph::verify::Context {
                    err: std::io::sink(),
                    out,
                    output_statistics: Some(crate::OutputFormat::Human),
                },
            )?;
        }
        Ok(())
    }
}
