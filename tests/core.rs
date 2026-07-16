use std::sync::Arc;

use bevy_semantics::{
    EdgeDirection, EdgeQuery, Kind, SemanticEdge, SemanticError, SemanticRegistry, Semantics,
    TraversalQuery,
};
use bevy_semantics::{SemanticCommand, SemanticPlaybackQueue};
use bevy_tasks::{futures_lite::future, AsyncComputeTaskPool};

#[test]
fn core_is_seeded_and_taxonomy_works() {
    let registry = SemanticRegistry::default();
    let core = registry.core();
    let relation = core.relation;
    let is_a = core.is_a;
    let not_a = core.not_a;

    assert_eq!(registry.name(relation), Some("Relation"));
    assert_eq!(registry.name(is_a), Some("is_a"));
    assert!(registry.has_edge(is_a, is_a, relation));
    assert!(registry.has_edge(not_a, is_a, relation));

    let snapshot = registry.snapshot();
    assert!(snapshot.is_a(is_a, relation));
    assert!(snapshot.is_a(relation, relation));
    assert_eq!(snapshot.targets(is_a, is_a), &[relation]);
}

#[test]
fn semantics_facade_queries_work() -> Result<(), SemanticError> {
    let mut semantics = Semantics::default();
    let core = semantics.core();
    let creature = semantics.register_kind("Creature")?;
    let beast = semantics.register_kind("Beast")?;
    let canine = semantics.register_kind("Canine")?;
    let wolf = semantics.register_kind("Wolf")?;
    let rabbit = semantics.register_kind("Rabbit")?;
    let preys_on = semantics.register_kind("preys_on")?;
    let predated_by = semantics.register_kind("predated_by")?;

    semantics.add_edge(beast, core.is_a, creature)?;
    semantics.add_edge(canine, core.is_a, beast)?;
    semantics.add_edge(wolf, core.is_a, canine)?;
    semantics.add_edge(wolf, preys_on, rabbit)?;
    semantics.add_edge(rabbit, predated_by, wolf)?;

    assert!(semantics.is_a(wolf, creature));
    assert_eq!(semantics.targets(wolf, preys_on), vec![rabbit]);
    assert_eq!(semantics.targets(rabbit, predated_by), vec![wolf]);
    assert_eq!(semantics.subjects(predated_by, wolf), vec![rabbit]);

    let mut lineage = vec![wolf];
    lineage.extend(semantics.reachable(wolf, core.is_a, EdgeDirection::Outgoing, usize::MAX));
    assert_eq!(lineage, vec![wolf, canine, beast, creature]);
    Ok(())
}

#[test]
fn fluent_edit_chain_supports_typed_and_bulk_edges() -> Result<(), SemanticError> {
    #[derive(Debug)]
    struct Marker;
    #[derive(Debug)]
    struct Creature;
    #[derive(Debug)]
    struct Beast;
    #[derive(Debug)]
    struct Canine;
    #[derive(Debug)]
    struct Wolf;

    let mut semantics = Semantics::default();
    let core = semantics.core();

    semantics
        .edit()
        .typed_kind_named::<Marker>("Marker")
        .typed_kind_named::<Creature>("Creature")
        .typed_kind_named::<Beast>("Beast")
        .typed_kind_named::<Canine>("Canine")
        .typed_kind_named::<Wolf>("Wolf")
        .register_kind("damage")
        .expect("seed typed kinds");

    let marker = semantics.kind_of::<Marker>()?;
    let creature = semantics.kind_of::<Creature>()?;
    let beast = semantics.kind_of::<Beast>()?;
    let canine = semantics.kind_of::<Canine>()?;
    let wolf = semantics.kind_of::<Wolf>()?;
    let damage = semantics.kind("damage")?;

    semantics
        .edit()
        .add_edges([
            (beast, core.is_a, creature),
            (canine, core.is_a, beast),
            (wolf, core.is_a, canine),
            (wolf, damage, marker),
        ])
        .expect("seed edit chain");

    let snapshot = semantics.snapshot();
    assert_eq!(snapshot.kind("damage"), Some(damage));
    assert!(snapshot.has_edge(wolf, damage, marker));
    assert!(snapshot.is_a(wolf, creature));
    Ok(())
}

#[test]
fn same_name_registration_is_stable() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let before = registry.version();

    let first = registry.register_kind("  Foo  ")?;
    let after_first = registry.version();
    let second = registry.register_kind("Foo")?;

    assert_eq!(first, second);
    assert_eq!(registry.kind("Foo"), Some(first));
    assert_eq!(registry.kind("  Foo  "), Some(first));
    assert!(after_first > before);
    assert_eq!(registry.version(), after_first);
    Ok(())
}

#[test]
fn bulk_edge_registration_is_transactional_and_idempotent() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let relation = registry.register_kind("related_to")?;
    let a = registry.register_kind("A")?;
    let b = registry.register_kind("B")?;
    let c = registry.register_kind("C")?;
    let unknown = Kind::from(u64::MAX);

    let error = registry
        .add_edges([(a, relation, b), (b, relation, unknown)])
        .expect_err("all edge tuples should validate before insertion");
    assert!(matches!(error, SemanticError::UnknownKind { kind } if kind == unknown));
    assert!(!registry.has_edge(a, relation, b));

    assert!(registry.add_edges([(a, relation, b), (b, relation, c)])?);
    let version = registry.version();
    assert!(!registry.add_edges([(a, relation, b), (b, relation, c)])?);
    assert_eq!(registry.version(), version);
    Ok(())
}

#[test]
fn typed_registration_tracks_type_id() -> Result<(), SemanticError> {
    #[derive(Debug)]
    struct Marker;

    let mut registry = SemanticRegistry::default();
    let kind = registry.typed_kind_named::<Marker>("Marker")?;

    assert_eq!(registry.kind_of::<Marker>(), Some(kind));
    assert_eq!(
        registry.type_id(kind),
        Some(std::any::TypeId::of::<Marker>())
    );
    assert_eq!(registry.name(kind), Some("Marker"));
    Ok(())
}

#[test]
fn unregister_kind_removes_incident_edges_and_type_bindings() -> Result<(), SemanticError> {
    #[derive(Debug)]
    struct Marker;

    let mut registry = SemanticRegistry::default();
    let relation = registry.register_kind("related_to")?;
    let source = registry.register_kind("Source")?;
    let target = registry.typed_kind_named::<Marker>("Marker")?;

    registry.add_edge(source, relation, target)?;
    registry.add_edge(target, relation, source)?;
    registry.add_edge(source, target, source)?;

    assert_eq!(registry.kind_of::<Marker>(), Some(target));
    assert_eq!(
        registry.type_id(target),
        Some(std::any::TypeId::of::<Marker>())
    );
    assert_eq!(registry.name(target), Some("Marker"));

    assert!(registry.unregister_kind(target)?);

    assert_eq!(registry.kind_of::<Marker>(), None);
    assert_eq!(registry.type_id(target), None);
    assert_eq!(registry.name(target), None);
    assert!(!registry.has_edge(source, relation, target));
    assert!(!registry.has_edge(target, relation, source));
    assert!(!registry.has_edge(source, target, source));

    let snapshot = registry.snapshot();
    assert!(!snapshot.has_edge(source, relation, target));
    assert!(!snapshot.has_edge(target, relation, source));
    assert!(!snapshot.has_edge(source, target, source));
    Ok(())
}

#[test]
fn core_kinds_cannot_be_unregistered() {
    let mut registry = SemanticRegistry::default();
    let core = registry.core();

    let error = registry
        .unregister_kind(core.is_a)
        .expect_err("built-ins should be protected");

    assert!(matches!(
        error,
        SemanticError::CannotUnregisterCoreKind { kind } if kind == core.is_a
    ));
}

#[test]
fn unregister_namespace_removes_matching_kinds_and_edges() -> Result<(), SemanticError> {
    #[derive(Debug)]
    struct Wolf;

    let mut semantics = Semantics::default();
    let core = semantics.core();

    let creature = semantics.register_kind("game::Creature")?;
    let beast = semantics.register_kind("game::Beast")?;
    let canine = semantics.register_kind("game::Canine")?;
    let wolf = semantics.typed_kind_named::<Wolf>("game::Wolf")?;
    let rabbit = semantics.register_kind("game::Rabbit")?;
    let preys_on = semantics.register_kind("game::preys_on")?;
    let stone = semantics.register_kind("other::Stone")?;
    let gameplay_marker = semantics.register_kind("gameplay::Marker")?;

    semantics.add_edge(beast, core.is_a, creature)?;
    semantics.add_edge(canine, core.is_a, beast)?;
    semantics.add_edge(wolf, core.is_a, canine)?;
    semantics.add_edge(wolf, preys_on, rabbit)?;
    semantics.add_edge(stone, core.is_a, stone)?;

    assert!(semantics.unregister_namespace("game")?);

    for name in [
        "game::Creature",
        "game::Beast",
        "game::Canine",
        "game::Wolf",
        "game::Rabbit",
        "game::preys_on",
    ] {
        assert!(matches!(
            semantics.kind(name),
            Err(SemanticError::UnknownKind { .. })
        ));
    }
    assert_eq!(semantics.kind("other::Stone")?, stone);
    assert_eq!(semantics.kind("gameplay::Marker")?, gameplay_marker);
    assert!(matches!(
        semantics.kind_of::<Wolf>(),
        Err(SemanticError::UnknownKind { .. })
    ));
    assert_eq!(semantics.name(wolf), None);
    assert_eq!(semantics.type_id(wolf), None);
    assert!(!semantics.has_edge(beast, core.is_a, creature));
    assert!(!semantics.has_edge(canine, core.is_a, beast));
    assert!(!semantics.has_edge(wolf, core.is_a, canine));
    assert!(!semantics.has_edge(wolf, preys_on, rabbit));
    assert!(semantics.has_edge(stone, core.is_a, stone));
    Ok(())
}

#[test]
fn unregister_namespace_command_is_supported() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let other = registry.register_kind("other::Stone")?;
    let game_kind = registry.register_kind("game::Creature")?;

    assert!(
        registry.apply_command(SemanticCommand::UnregisterNamespace {
            namespace: Arc::from("game"),
        })?
    );

    assert!(registry.kind("game::Creature").is_none());
    assert_eq!(registry.kind("other::Stone"), Some(other));
    assert_eq!(registry.name(game_kind), None);
    Ok(())
}

#[test]
fn core_namespace_is_reserved_case_insensitively() {
    #[derive(Debug)]
    struct Marker;

    let mut registry = SemanticRegistry::default();

    let error = registry
        .register_kind("CORE::Wolf")
        .expect_err("reserved namespace should be blocked");
    assert!(matches!(
        error,
        SemanticError::CannotUseReservedNamespace { namespace } if namespace.eq_ignore_ascii_case("core")
    ));

    let error = registry
        .register_kind("core")
        .expect_err("reserved namespace should be blocked");
    assert!(matches!(
        error,
        SemanticError::CannotUseReservedNamespace { namespace } if namespace.eq_ignore_ascii_case("core")
    ));

    let error = registry
        .typed_kind_named::<Marker>("Core::Marker")
        .expect_err("reserved namespace should be blocked");
    assert!(matches!(
        error,
        SemanticError::CannotUseReservedNamespace { namespace } if namespace.eq_ignore_ascii_case("core")
    ));

    let error = registry
        .unregister_namespace("core")
        .expect_err("reserved namespace should be blocked");
    assert!(matches!(
        error,
        SemanticError::CannotUseReservedNamespace { namespace } if namespace.eq_ignore_ascii_case("core")
    ));
}

#[test]
fn unregister_kind_command_is_supported() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let relation = registry.register_kind("related_to")?;
    let source = registry.register_kind("Source")?;
    let target = registry.register_kind("Target")?;

    registry.add_edge(source, relation, target)?;
    assert!(registry.apply_command(SemanticCommand::UnregisterKind { kind: target })?);
    assert!(!registry.has_edge(source, relation, target));
    Ok(())
}

#[test]
fn edge_queries_and_traversal_are_deterministic() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let snapshot = {
        let mut batch = registry.batch();
        let core = batch.core();
        let is_a = core.is_a;

        let a = batch.register_kind("A")?;
        let b = batch.register_kind("B")?;
        let c = batch.register_kind("C")?;
        batch.add_edges([(a, is_a, b), (b, is_a, c)])?;

        batch.snapshot()
    };

    let core = registry.core();
    let is_a = core.is_a;
    let a = registry.kind("A").expect("A kind");
    let b = registry.kind("B").expect("B kind");
    let c = registry.kind("C").expect("C kind");

    assert_eq!(snapshot.targets(a, is_a), &[b]);
    assert_eq!(snapshot.subjects(is_a, b), &[a]);
    assert_eq!(
        snapshot.reachable(a, is_a, EdgeDirection::Outgoing, 2),
        vec![b, c]
    );
    assert!(snapshot.is_a(a, c));

    let edge = SemanticEdge::new(a, is_a, b);
    assert_eq!(
        snapshot
            .edge_query()
            .subject(a)
            .relation(is_a)
            .target(b)
            .run_edges(&snapshot)?,
        vec![edge]
    );
    assert_eq!(
        snapshot
            .edge_query()
            .subject(a)
            .relation(is_a)
            .target(b)
            .run_count(&snapshot)?,
        1
    );
    assert!(snapshot
        .edge_query()
        .subject(a)
        .relation(is_a)
        .target(b)
        .run_exists(&snapshot)?);
    assert_eq!(
        snapshot
            .edge_query()
            .relation(is_a)
            .target(c)
            .run_subjects(&snapshot)?,
        vec![b]
    );
    assert_eq!(
        snapshot
            .traversal_query()
            .seed(a)
            .relation(is_a)
            .depth_exact(1)
            .run_kinds(&snapshot)?,
        vec![b]
    );
    assert_eq!(
        snapshot
            .traversal_query()
            .seed(a)
            .relation(is_a)
            .include_start(true)
            .depth_to(2)
            .run_kinds(&snapshot)?,
        vec![a, b, c]
    );
    Ok(())
}

#[test]
fn batch_builds_snapshot_once() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let is_a = registry.core().is_a;

    let snapshot = {
        let mut batch = registry.batch();
        let a = batch.register_kind("A")?;
        let b = batch.register_kind("B")?;
        batch.add_edge(a, is_a, b)?;
        batch.snapshot()
    };

    let a = registry.kind("A").expect("A kind");
    let b = registry.kind("B").expect("B kind");
    assert!(snapshot.has_edge(a, is_a, b));
    assert!(snapshot.is_a(a, b));
    Ok(())
}

#[test]
fn compiled_queries_fail_when_snapshot_versions_change() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let is_a = registry.core().is_a;
    let a = registry.register_kind("A")?;
    let b = registry.register_kind("B")?;

    registry.add_edge(a, is_a, b)?;
    let snapshot = registry.snapshot();

    let compiled = snapshot
        .edge_query()
        .subject(a)
        .relation(is_a)
        .target(b)
        .compile(&snapshot)?;

    registry.register_kind("C")?;
    let newer_snapshot = registry.snapshot();

    let error = compiled.run_exists(&newer_snapshot).unwrap_err();
    assert!(matches!(
        error,
        SemanticError::StaleCompiledQuery {
            domain: "edge query",
            ..
        }
    ));
    Ok(())
}

#[test]
fn snapshot_arc_reuses_cached_snapshot_until_mutation() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let first = registry.snapshot_cached();
    let second = registry.snapshot_cached();
    assert!(Arc::ptr_eq(&first, &second));

    let _ = registry.register_kind("cached_kind")?;
    let third = registry.snapshot_cached();
    assert!(!Arc::ptr_eq(&first, &third));
    assert_eq!(third.version(), registry.version());
    Ok(())
}

#[test]
fn cloned_registries_keep_independent_snapshot_caches() -> Result<(), SemanticError> {
    let registry = SemanticRegistry::default();
    let _ = registry.snapshot_cached();
    let mut left = registry.clone();
    let mut right = registry.clone();

    let left_only = left.register_kind("left_only")?;
    let right_only = right.register_kind("right_only")?;
    assert_eq!(left.version(), right.version());

    let left_snapshot = left.snapshot_cached();
    let right_snapshot = right.snapshot_cached();
    assert_eq!(left_snapshot.kind("left_only"), Some(left_only));
    assert_eq!(left_snapshot.kind("right_only"), None);
    assert_eq!(right_snapshot.kind("right_only"), Some(right_only));
    assert_eq!(right_snapshot.kind("left_only"), None);
    Ok(())
}

#[test]
fn query_caches_are_scoped_to_registry_lineage() -> Result<(), SemanticError> {
    let mut base = SemanticRegistry::default();
    let relation = base.register_kind("related_to")?;
    let a = base.register_kind("A")?;
    let b = base.register_kind("B")?;
    let c = base.register_kind("C")?;
    let mut left = base.clone();
    let mut right = base.clone();
    left.add_edge(a, relation, b)?;
    right.add_edge(a, relation, c)?;
    assert_eq!(left.version(), right.version());

    let left_snapshot = left.snapshot();
    let right_snapshot = right.snapshot();
    let edge_query = EdgeQuery::builder().subject(a).relation(relation);
    assert_eq!(edge_query.run_targets(&left_snapshot)?, vec![b]);
    assert_eq!(edge_query.run_targets(&right_snapshot)?, vec![c]);

    let compiled_edge = edge_query.compile(&left_snapshot)?;
    assert!(matches!(
        compiled_edge.run_edges(&right_snapshot),
        Err(SemanticError::SnapshotLineageMismatch {
            domain: "edge query"
        })
    ));
    assert!(matches!(
        left_snapshot.edge_query().run_edges(&right_snapshot),
        Err(SemanticError::SnapshotLineageMismatch {
            domain: "edge query"
        })
    ));

    let traversal_query = TraversalQuery::builder()
        .seed(a)
        .relation(relation)
        .depth_1();
    assert_eq!(traversal_query.run_kinds(&left_snapshot)?, vec![b]);
    assert_eq!(traversal_query.run_kinds(&right_snapshot)?, vec![c]);

    let compiled_traversal = traversal_query.compile(&left_snapshot)?;
    assert!(matches!(
        compiled_traversal.run_kinds(&right_snapshot),
        Err(SemanticError::SnapshotLineageMismatch {
            domain: "traversal query"
        })
    ));
    assert!(matches!(
        left_snapshot
            .traversal_query()
            .seed(a)
            .run_kinds(&right_snapshot),
        Err(SemanticError::SnapshotLineageMismatch {
            domain: "traversal query"
        })
    ));
    Ok(())
}

#[test]
fn task_commands_can_be_played_back_later() -> Result<(), SemanticError> {
    let _ = AsyncComputeTaskPool::get_or_init(bevy_tasks::TaskPool::new);

    let mut registry = SemanticRegistry::default();
    let snapshot = {
        let mut batch = registry.batch();
        let core = batch.core();
        let creature = batch.register_kind("Creature")?;
        let beast = batch.register_kind("Beast")?;
        let canine = batch.register_kind("Canine")?;
        let wolf = batch.register_kind("Wolf")?;
        let rabbit = batch.register_kind("Rabbit")?;
        let item = batch.register_kind("Item")?;
        let resource = batch.register_kind("Resource")?;
        let herb = batch.register_kind("Herb")?;
        let loot = batch.register_kind("Loot")?;
        let pelt = batch.register_kind("Pelt")?;
        let place = batch.register_kind("Place")?;
        let forest = batch.register_kind("Forest")?;
        let preys_on = batch.register_kind("preys_on")?;
        let predated_by = batch.register_kind("predated_by")?;
        let drops = batch.register_kind("drops")?;
        let dropped_by = batch.register_kind("dropped_by")?;
        let grows_in = batch.register_kind("grows_in")?;

        for relation in [preys_on, predated_by, drops, dropped_by, grows_in] {
            batch.add_edge(relation, core.is_a, core.relation)?;
        }

        batch.add_edge(preys_on, core.inverse_of, predated_by)?;
        batch.add_edge(predated_by, core.inverse_of, preys_on)?;
        batch.add_edge(drops, core.inverse_of, dropped_by)?;
        batch.add_edge(dropped_by, core.inverse_of, drops)?;

        batch.add_edge(beast, core.is_a, creature)?;
        batch.add_edge(canine, core.is_a, beast)?;
        batch.add_edge(wolf, core.is_a, canine)?;
        batch.add_edge(rabbit, core.is_a, beast)?;
        batch.add_edge(resource, core.is_a, item)?;
        batch.add_edge(herb, core.is_a, resource)?;
        batch.add_edge(loot, core.is_a, item)?;
        batch.add_edge(pelt, core.is_a, loot)?;
        batch.add_edge(forest, core.is_a, place)?;

        batch.add_edge(wolf, preys_on, rabbit)?;
        batch.add_edge(wolf, drops, pelt)?;
        batch.add_edge(herb, grows_in, forest)?;

        batch.snapshot()
    };

    let preys_on = registry.kind("preys_on").expect("preys_on relation");
    let predated_by = registry.kind("predated_by").expect("predated_by relation");
    let drops = registry.kind("drops").expect("drops relation");
    let dropped_by = registry.kind("dropped_by").expect("dropped_by relation");
    let rabbit = registry.kind("Rabbit").expect("Rabbit kind");
    let pelt = registry.kind("Pelt").expect("Pelt kind");
    let wolf = registry.kind("Wolf").expect("Wolf kind");

    let mut task = AsyncComputeTaskPool::get().spawn(async move {
        // `sq_` marks semantic query results derived from the snapshot.
        let sq_prey_edges = snapshot
            .edge_query()
            .relation(preys_on)
            .run_edges(&snapshot)
            .expect("preys_on edges");
        let sq_drop_edges = snapshot
            .edge_query()
            .relation(drops)
            .run_edges(&snapshot)
            .expect("drops edges");

        let mut edges = Vec::with_capacity(sq_prey_edges.len() + sq_drop_edges.len());
        edges.extend(
            sq_prey_edges
                .into_iter()
                .map(|edge| (edge.target, predated_by, edge.subject)),
        );
        edges.extend(
            sq_drop_edges
                .into_iter()
                .map(|edge| (edge.target, dropped_by, edge.subject)),
        );
        vec![SemanticCommand::AddEdges { edges }]
    });

    let commands = loop {
        if let Some(commands) = future::block_on(future::poll_once(&mut task)) {
            break commands;
        }
    };

    let mut playback = SemanticPlaybackQueue::default();
    playback.enqueue(commands);
    playback.playback(&mut registry)?;

    let snapshot = registry.snapshot();
    let sq_predators = snapshot
        .edge_query()
        .subject(rabbit)
        .relation(predated_by)
        .run_targets(&snapshot)?;
    let sq_droppers = snapshot
        .edge_query()
        .subject(pelt)
        .relation(dropped_by)
        .run_targets(&snapshot)?;
    assert_eq!(sq_predators, vec![wolf]);
    assert_eq!(sq_droppers, vec![wolf]);
    assert!(snapshot.kind("Creature").is_some());
    assert!(playback.is_empty());
    Ok(())
}
