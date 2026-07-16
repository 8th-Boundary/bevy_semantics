use std::sync::Arc;

use bevy_ecs::component::Component;
use bevy_ecs::world::World;
use bevy_semantics::core::{CAN_BE, IS_A};
use bevy_semantics::{
    semantic_kinds, Kind, SemanticCommand, SemanticComponent, SemanticComponents,
    SemanticPlaybackQueue, SemanticRegistry, SemanticWorldExt,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const NODE_COUNT: usize = 4_096;
const LINK_STRIDE: usize = 16;
const TRAVERSAL_DEPTH: usize = 128;

semantic_kinds! {
    const BENCH_CONST_KINDS = {
        CONST_KIND_A = "ConstKindA",
        CONST_KIND_B = "ConstKindB",
        CONST_KIND_C = "ConstKindC",
        CONST_KIND_D = "ConstKindD",
        CONST_RELATION_A = "const_relation_a",
        CONST_RELATION_B = "const_relation_b",
        CONST_RELATION_C = "const_relation_c",
        CONST_RELATION_D = "const_relation_d",
    };
}

#[derive(Component, SemanticComponent)]
#[semantic(kind = "BenchComponent")]
struct BenchComponent;

struct Fixture {
    registry: SemanticRegistry,
    snapshot: Arc<bevy_semantics::SemanticSnapshot>,
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

fn populate_registry(registry: &mut SemanticRegistry) -> (Kind, Kind, Kind, Kind, Vec<Kind>) {
    let mut batch = registry.batch();
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
            .add_edge(nodes[index - 1], IS_A, node)
            .expect("chain edge");
    }

    let linked_edges = (0..NODE_COUNT)
        .step_by(LINK_STRIDE)
        .map(|index| (root, linked_to, nodes[index]))
        .collect::<Vec<_>>();
    batch.add_edges(&linked_edges).expect("linked edges");

    (IS_A, root, linked_to, playable, nodes)
}

fn derive_task_commands(
    snapshot: &bevy_semantics::SemanticSnapshot,
    root: Kind,
    relation: Kind,
    target: Kind,
) -> Vec<SemanticCommand> {
    let descendants = snapshot
        .traversal_query()
        .seed(root)
        .relation(IS_A)
        .transitive()
        .run_kinds(snapshot)
        .expect("descendants");

    let edges = descendants
        .into_iter()
        .map(|subject| (subject, relation, target))
        .collect();
    vec![SemanticCommand::AddEdges { edges }]
}

fn build_fixture() -> Fixture {
    let mut registry = SemanticRegistry::default();
    let (is_a, root, linked_to, playable, nodes) = populate_registry(&mut registry);
    let snapshot = registry.snapshot_cached();
    let lookup_index = NODE_COUNT / 2;
    let lookup_name = format!("node_{lookup_index:04}");
    let target = nodes[lookup_index];
    let task_commands = derive_task_commands(&snapshot, root, CAN_BE, playable);
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

fn bench_snapshot_build_tombstoned(c: &mut Criterion) {
    let mut fixture = build_fixture();
    for index in (LINK_STRIDE..NODE_COUNT).step_by(128) {
        let name = format!("node_{index:04}");
        let kind = fixture.registry.kind(&name).expect("tombstoned node kind");
        fixture
            .registry
            .tombstone_kind(kind)
            .expect("tombstone benchmark node");
    }

    c.bench_function("semantics_snapshot_build_4k_tombstoned", |b| {
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

fn bench_const_registration(c: &mut Criterion) {
    c.bench_function("semantics_const_registration_8", |b| {
        b.iter_batched(
            SemanticRegistry::default,
            |mut registry| {
                registry
                    .register_consts(black_box(BENCH_CONST_KINDS))
                    .expect("compile-time kind group registration");
                black_box(registry);
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_semantic_component_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantics_component_registration");

    group.bench_function("cold_world", |b| {
        b.iter_batched(
            World::new,
            |mut world| {
                black_box(
                    world
                        .try_register_semantic_component::<BenchComponent>()
                        .expect("cold semantic component registration"),
                )
            },
            criterion::BatchSize::SmallInput,
        )
    });

    let mut world = World::new();
    world
        .try_register_semantic_component::<BenchComponent>()
        .expect("initial semantic component registration");

    group.bench_function("idempotent", |b| {
        b.iter(|| {
            black_box(
                world
                    .try_register_semantic_component::<BenchComponent>()
                    .expect("idempotent semantic component registration"),
            )
        })
    });

    group.bench_function("kind_to_component_lookup", |b| {
        let mappings = world.resource::<SemanticComponents>();
        b.iter(|| black_box(mappings.component_id(black_box(BenchComponent::KIND))))
    });

    group.finish();
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
                black_box(CAN_BE),
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
    bench_snapshot_build_tombstoned,
    bench_snapshot_cached,
    bench_bulk_authoring,
    bench_lookup_kind_by_name,
    bench_const_registration,
    bench_semantic_component_registration,
    bench_is_a_direct,
    bench_edge_query_dynamic,
    bench_edge_query_compiled,
    bench_traversal_dynamic,
    bench_traversal_compiled,
    bench_task_command_derivation,
    bench_task_command_playback
);
criterion_main!(semantics_benches);
