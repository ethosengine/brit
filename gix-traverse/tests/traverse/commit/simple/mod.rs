use crate::Result;
use crate::hex_to_id;
use crate::util::{commit_graph, git_graph, git_graph_with_time, named_fixture, parse_commit_names};
use gix_hash::ObjectId;
use gix_traverse::commit::{Parents, Simple, simple::Sorting};

mod adjusted_dates;
mod different_date;
mod different_date_intermixed;
mod hide;
mod same_date;

/// Run a simple traversal and collect the resulting commit IDs.
fn traverse(
    tips: impl IntoIterator<Item = ObjectId>,
    odb: &gix_odb::Handle,
    sorting: Sorting,
    parents: Parents,
    hidden: impl IntoIterator<Item = ObjectId>,
) -> Result<Vec<ObjectId>> {
    let tips: Vec<_> = tips.into_iter().collect();
    let hidden: Vec<_> = hidden.into_iter().collect();
    let graph = commit_graph(odb.store_ref());
    let actual = Simple::new(tips.clone(), odb)
        .sorting(sorting)?
        .parents(parents)
        .commit_graph(graph)
        .hide(hidden.clone())?
        .map(|res| res.map(|info| info.id))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mapped = mapped_traversal(tips, odb, sorting, parents, hidden, commit_graph(odb.store_ref()))?;
    assert_eq!(actual, mapped, "immutable storage preserves traversal order");
    Ok(actual)
}

/// Run a traversal with both commit-graph enabled and disabled to ensure consistency.
fn traverse_both(
    tips: impl IntoIterator<Item = ObjectId> + Clone,
    odb: &gix_odb::Handle,
    sorting: Sorting,
    parents: Parents,
    hidden: impl IntoIterator<Item = ObjectId> + Clone,
) -> Result<Vec<ObjectId>> {
    // Without commit graph
    let without_graph: Vec<_> = Simple::new(tips.clone(), odb)
        .sorting(sorting)?
        .parents(parents)
        .commit_graph(None)
        .hide(hidden.clone())?
        .map(|res| res.map(|info| info.id))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // With commit graph
    let graph = commit_graph(odb.store_ref());
    let with_graph: Vec<_> = Simple::new(tips.clone(), odb)
        .sorting(sorting)?
        .parents(parents)
        .commit_graph(graph)
        .hide(hidden.clone())?
        .map(|res| res.map(|info| info.id))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    assert_eq!(
        without_graph, with_graph,
        "results must be consistent with and without commit-graph"
    );
    for graph in [None, commit_graph(odb.store_ref())] {
        assert_eq!(
            with_graph,
            mapped_traversal(tips.clone(), odb, sorting, parents, hidden.clone(), graph)?,
            "mapped storage preserves cached and uncached traversal order"
        );
    }
    Ok(with_graph)
}

fn mapped_traversal(
    tips: impl IntoIterator<Item = ObjectId>,
    odb: &gix_odb::Handle,
    sorting: Sorting,
    parents: Parents,
    hidden: impl IntoIterator<Item = ObjectId>,
    graph: Option<gix_commitgraph::Graph>,
) -> Result<Vec<ObjectId>> {
    let scratch = gix_testtools::tempfile::tempdir()?;
    let budget = gix_utils::stream::Budget::new(scratch.path())?;
    let actual = Simple::new(
        tips,
        super::MappedSource {
            odb,
            budget: budget.clone(),
        },
    )
    .sorting(sorting)?
    .parents(parents)
    .commit_graph(graph)
    .hide(hidden)?
    .map(|res| res.map(|info| info.id))
    .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(budget.usage().0, 0, "walk and hidden-frontier mappings are reclaimed");
    Ok(actual)
}

fn all_sortings() -> impl Iterator<Item = Sorting> {
    use gix_traverse::commit::simple::CommitTimeOrder;
    [
        Sorting::BreadthFirst,
        Sorting::ByCommitTime(CommitTimeOrder::NewestFirst),
        Sorting::ByCommitTime(CommitTimeOrder::OldestFirst),
    ]
    .into_iter()
}
