use gix::{
    Result,
    diff::blob::ResourceKind,
    error::{OptionExt, ResultExt, message},
    filter::plumbing::{driver::apply::Delay, pipeline::convert::to_worktree},
    objs::FindStream,
    revision::Spec,
    utils::stream::{self, Budget},
};

use crate::repository::revision::resolve::{BlobFormat, TreeMode};

pub fn display_object(
    repo: &gix::Repository,
    spec: Spec<'_>,
    tree_mode: TreeMode,
    cache: Option<(BlobFormat, &mut gix::diff::blob::Platform)>,
    mut out: impl std::io::Write,
) -> Result<()> {
    let id = spec
        .single()
        .ok_or_raise(|| message("rev-spec must resolve to a single object").validation())?;
    let header = id.header()?;
    match header.kind() {
        gix::object::Kind::Tree if matches!(tree_mode, TreeMode::Pretty) => {
            for entry in id.object()?.into_tree().iter() {
                writeln!(out, "{}", entry?).or_error()?;
            }
        }
        gix::object::Kind::Blob if cache.is_some() && spec.path_and_mode().is_some() => {
            let (path, mode) = spec.path_and_mode().expect("is present");
            match cache.expect("is some") {
                (BlobFormat::Git, _) => unreachable!("no need for a cache when querying object db"),
                (BlobFormat::Worktree, cache) => {
                    let platform = cache
                        .attr_stack
                        .at_entry(path, Some(mode.into()), &repo.objects)
                        .or_error()?;
                    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
                    let (_, object) = repo
                        .objects
                        .find_stream(&id, &budget, &gix::interrupt::IS_INTERRUPTED)?;
                    let converted = cache.filter.worktree_filter.convert_to_worktree_stream(
                        object,
                        path,
                        &mut |_path, attrs| {
                            let _ = platform.matching_attributes(attrs);
                        },
                        to_worktree::Options {
                            can_delay: Delay::Forbid,
                            unknown_encoding: to_worktree::UnknownEncoding::Fail,
                        },
                        &budget,
                        &gix::interrupt::IS_INTERRUPTED,
                    )?;
                    let gix::filter::plumbing::pipeline::stream::Outcome::Ready(mut converted) = converted else {
                        unreachable!("delay is forbidden")
                    };
                    stream::copy(&mut converted, &mut out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
                }
                (BlobFormat::Diff | BlobFormat::DiffOrGit, cache) => {
                    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
                    cache.set_resource_bounded(
                        id.detach(),
                        mode.kind(),
                        path,
                        ResourceKind::OldOrSource,
                        &repo.objects,
                        &budget,
                        repo.allocation_limit()
                            .unwrap_or(usize::try_from(repo.big_file_threshold()?).unwrap_or(usize::MAX)),
                        &gix::interrupt::IS_INTERRUPTED,
                    )?;
                    let resource = cache.resource(ResourceKind::OldOrSource).expect("just set");
                    let data = resource
                        .data
                        .as_slice()
                        .ok_or_raise(|| message!("Binary data at {path} cannot be diffed").unsupported())?;
                    stream::copy(&mut &data[..], &mut out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
                    cache.clear_resource_cache();
                }
            }
        }
        _ => {
            let budget = Budget::new(&std::env::temp_dir()).or_error()?;
            let (_, mut object) = repo
                .objects
                .find_stream(&id, &budget, &gix::interrupt::IS_INTERRUPTED)?;
            stream::copy(&mut object, &mut out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
        }
    }
    Ok(())
}

pub(super) mod function {
    use crate::repository::revision::resolve::TreeMode;
    use gix::Result;

    pub fn cat(repo: gix::Repository, revspec: &str, out: impl std::io::Write) -> Result<()> {
        super::display_object(&repo, repo.rev_parse(revspec)?, TreeMode::Pretty, None, out)?;
        Ok(())
    }
}
