use std::sync::Arc;

use bevy_semantics::{
    Core, Kind, SemanticCommand, SemanticPlaybackQueue, SemanticRegistry, Weight,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const NODE_COUNT: usize = 4_096;
const LINK_STRIDE: usize = 16;
const TRAVERSAL_DEPTH: usize = 128;

struct Fixture {
    registry: SemanticRegistry,
    snapshot: Arc<bevy_semantics::SemanticSnapshot>,
    core: Core,
    root: Kind,
    target: Kind,
    playable: Kind,
    lookup_name: String,
    edge_query: bevy_semantics::EdgeQueryBuilder,
    compiled_edge_query: bevy_semantics::CompiledEdgeQuery,
    traversal_query: bevy_semantics::TraversalQueryBuilder,
    compiled_traversal_query: bevy_semantics::CompiledTraversalQuery,
    task_commands: Vec<SemanticCommand>,
}

fn populate_registry(registry: &mut SemanticRegistry) -> (Core, Kind, Kind, Kind, Kind, Vec<Kind>) {
    let mut batch = registry.batch();
    let core = batch.core();
    let is_a = core.is_a;
    let root = batch.register_kind("root").expect("root kind");
    let linked_to = batch.register_kind("linked_to").expect("linked_to kind");
    let playable = batch.register_kind("Playable").expect("Playable kind");

    let mut nodes = Vec::with_capacity(NODE_COUNT);
    nodes.push(root);
    for index in 1..NODE_COUNT {
        let node = batch
            .register_kind(format!("node_{index:04}"))
            .expect("node kind");
        nodes.push(node);
        batch
            .add_edge(nodes[index - 1], is_a, node)
            .expect("chain edge");
    }

    for index in (0..NODE_COUNT).step_by(LINK_STRIDE) {
        batch
            .add_edge_weighted(root, linked_to, nodes[index], Weight::from(index as i64))
            .expect("linked edge");
    }

    (core, is_a, root, linked_to, playable, nodes)
}

fn derive_task_commands(
    snapshot: &bevy_semantics::SemanticSnapshot,
    root: Kind,
    relation: Kind,
    target: Kind,
) -> Vec<SemanticCommand> {
    let descendants = snapshot
        .edge_query()
        .relation(snapshot.core().is_a)
        .target(root)
        .run_subjects(snapshot)
        .expect("descendants");

    descendants
        .into_iter()
        .map(|subject| SemanticCommand::AddEdge {
            subject,
            relation,
            target,
            weight: None,
        })
        .collect()
}

fn build_fixture() -> Fixture {
    let mut registry = SemanticRegistry::default();
    let (core, is_a, root, linked_to, playable, nodes) = populate_registry(&mut registry);
    let snapshot = registry.snapshot_cached();
    let lookup_index = NODE_COUNT / 2;
    let lookup_name = format!("node_{lookup_index:04}");
    let target = nodes[lookup_index];
    let task_commands = derive_task_commands(&snapshot, root, core.can_be, playable);
    let subject_source = snapshot
        .traversal_query()
        .seed(root)
        .relation(is_a)
        .depth_to(TRAVERSAL_DEPTH)
        .include_start(true);
    let edge_query = snapshot
        .edge_query()
        .relation(linked_to)
        .subjects_from(subject_source.clone())
        .targets_from(vec![target]);
    let compiled_edge_query = edge_query
        .clone()
        .compile(&snapshot)
        .expect("compile edge query");
    let traversal_query = snapshot
        .traversal_query()
        .seed(root)
        .relation(is_a)
        .depth_to(TRAVERSAL_DEPTH)
        .include_start(true);
    let compiled_traversal_query = traversal_query
        .clone()
        .compile(&snapshot)
        .expect("compile traversal query");

    Fixture {
        registry,
        snapshot,
        core,
        root,
        target,
        playable,
        lookup_name,
        edge_query,
        compiled_edge_query,
        traversal_query,
        compiled_traversal_query,
        task_commands,
    }
}

fn build_bulk_authoring_snapshot() -> bevy_semantics::SemanticSnapshot {
    let mut registry = SemanticRegistry::default();
    let _ = populate_registry(&mut registry);
    registry.snapshot()
}

fn bench_snapshot_build(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_snapshot_build_4k", |b| {
        b.iter(|| black_box(fixture.registry.snapshot()))
    });
}

fn bench_snapshot_cached(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_snapshot_cached_4k", |b| {
        b.iter(|| black_box(fixture.registry.snapshot_cached()))
    });
}

fn bench_bulk_authoring(c: &mut Criterion) {
    c.bench_function("semantics_bulk_authoring_4k", |b| {
        b.iter(|| black_box(build_bulk_authoring_snapshot()))
    });
}

fn bench_lookup_kind_by_name(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_lookup_kind_by_name", |b| {
        b.iter(|| black_box(fixture.snapshot.kind(black_box(&fixture.lookup_name))))
    });
}

fn bench_is_a_direct(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_is_a_direct", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .snapshot
                    .as_ref()
                    .is_a(black_box(fixture.root), black_box(fixture.target)),
            )
        })
    });
}

fn bench_edge_query_dynamic(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_edge_query_dynamic", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .edge_query
                    .run_edges(black_box(&fixture.snapshot))
                    .expect("dynamic edge query"),
            )
        })
    });
}

fn bench_edge_query_compiled(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_edge_query_compiled", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .compiled_edge_query
                    .run_edges(black_box(&fixture.snapshot))
                    .expect("compiled edge query"),
            )
        })
    });
}

fn bench_traversal_dynamic(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_traversal_dynamic", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .traversal_query
                    .run_kinds(black_box(&fixture.snapshot))
                    .expect("dynamic traversal query"),
            )
        })
    });
}

fn bench_traversal_compiled(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_traversal_compiled", |b| {
        b.iter(|| {
            black_box(
                fixture
                    .compiled_traversal_query
                    .run_kinds(black_box(&fixture.snapshot))
                    .expect("compiled traversal query"),
            )
        })
    });
}

fn bench_task_command_derivation(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_task_command_derivation_4k", |b| {
        b.iter(|| {
            black_box(derive_task_commands(
                black_box(&fixture.snapshot),
                black_box(fixture.root),
                black_box(fixture.core.can_be),
                black_box(fixture.playable),
            ))
        })
    });
}

fn bench_task_command_playback(c: &mut Criterion) {
    let fixture = build_fixture();
    c.bench_function("semantics_task_command_playback_4k", |b| {
        b.iter_batched(
            || {
                let registry = fixture.registry.clone();
                let mut playback = SemanticPlaybackQueue::default();
                playback.enqueue(fixture.task_commands.clone());
                (registry, playback)
            },
            |(mut registry, mut playback)| {
                black_box(
                    playback
                        .playback(&mut registry)
                        .expect("task command playback"),
                );
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    semantics_benches,
    bench_snapshot_build,
    bench_snapshot_cached,
    bench_bulk_authoring,
    bench_lookup_kind_by_name,
    bench_is_a_direct,
    bench_edge_query_dynamic,
    bench_edge_query_compiled,
    bench_traversal_dynamic,
    bench_traversal_compiled,
    bench_task_command_derivation,
    bench_task_command_playback
);
criterion_main!(semantics_benches);
