use bevy_app::{App, Last, Startup, Update};
use bevy_ecs::prelude::{Res, ResMut, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_semantics::core::{INVERSE_OF, IS_A, RELATION};
use bevy_semantics::prelude::*;
use bevy_tasks::{futures_lite::future, AsyncComputeTaskPool, Task, TaskPool};

// These IDs are available at compile time and can be shared by systems, tasks,
// and semantic components without consulting a runtime registry. The generated
// `&[KindRegistration]` tuple slice publishes all their names in one call.
semantic_kinds! {
    const DEMO_KINDS = {
        CREATURE = "Creature",
        BEAST = "Beast",
        CANINE = "Canine",
        WOLF = "Wolf",
        RABBIT = "Rabbit",
        ITEM = "Item",
        RESOURCE = "Resource",
        HERB = "Herb",
        LOOT = "Loot",
        PELT = "Pelt",
        PLACE = "Place",
        FOREST = "Forest",
        PREYS_ON = "preys_on",
        PREDATED_BY = "predated_by",
        DROPS = "drops",
        DROPPED_BY = "dropped_by",
        GROWS_IN = "grows_in",
    };
}

#[derive(Resource, Default)]
struct TaskState {
    task: Option<Task<Vec<SemanticCommand>>>,
}

#[derive(Resource, Default)]
struct ReportState {
    printed: bool,
}

fn setup_semantics(mut semantics: ResMut<Semantics>, mut task_state: ResMut<TaskState>) {
    // Keep the Bevy system itself non-fallible; the helper does the actual setup work.
    if let Err(error) = setup_semantics_impl(&mut semantics, &mut task_state) {
        panic!("failed to seed semantics: {error}");
    }
}

fn setup_semantics_impl(
    semantics: &mut Semantics,
    task_state: &mut TaskState,
) -> Result<(), SemanticError> {
    let registrations: &[KindRegistration] = DEMO_KINDS;
    semantics.register_consts(registrations)?;

    semantics.add_edges([
        (PREYS_ON, IS_A, RELATION),
        (PREDATED_BY, IS_A, RELATION),
        (DROPS, IS_A, RELATION),
        (DROPPED_BY, IS_A, RELATION),
        (GROWS_IN, IS_A, RELATION),
        (PREYS_ON, INVERSE_OF, PREDATED_BY),
        (PREDATED_BY, INVERSE_OF, PREYS_ON),
        (DROPS, INVERSE_OF, DROPPED_BY),
        (DROPPED_BY, INVERSE_OF, DROPS),
        (BEAST, IS_A, CREATURE),
        (CANINE, IS_A, BEAST),
        (WOLF, IS_A, CANINE),
        (RABBIT, IS_A, BEAST),
        (RESOURCE, IS_A, ITEM),
        (HERB, IS_A, RESOURCE),
        (LOOT, IS_A, ITEM),
        (PELT, IS_A, LOOT),
        (FOREST, IS_A, PLACE),
        (WOLF, PREYS_ON, RABBIT),
        (WOLF, DROPS, PELT),
        (HERB, GROWS_IN, FOREST),
    ])?;

    let snapshot = semantics.snapshot();
    task_state.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        // `sq_` marks semantic query results derived from the snapshot.
        let sq_prey_edges = snapshot
            .edge_query()
            .relation(PREYS_ON)
            .run_edges(&snapshot)
            .expect("preys_on edges");
        let sq_drop_edges = snapshot
            .edge_query()
            .relation(DROPS)
            .run_edges(&snapshot)
            .expect("drops edges");

        let mut edges = Vec::with_capacity(sq_prey_edges.len() + sq_drop_edges.len());
        edges.extend(
            sq_prey_edges
                .into_iter()
                .map(|edge| (edge.target, PREDATED_BY, edge.subject)),
        );
        edges.extend(
            sq_drop_edges
                .into_iter()
                .map(|edge| (edge.target, DROPPED_BY, edge.subject)),
        );
        vec![SemanticCommand::AddEdges { edges }]
    }));

    Ok(())
}

fn collect_task_result(
    mut task_state: ResMut<TaskState>,
    mut playback: ResMut<SemanticPlaybackQueue>,
) {
    if let Some(task) = task_state.task.as_mut() {
        if let Some(commands) = future::block_on(future::poll_once(task)) {
            task_state.task = None;
            playback.enqueue(commands);
        }
    }
}

fn playback_task_result(
    mut semantics: ResMut<Semantics>,
    mut playback: ResMut<SemanticPlaybackQueue>,
) {
    if let Err(error) = playback.playback(&mut semantics) {
        panic!("play back task commands: {error}");
    }
}

fn report_task_result(
    semantics: Res<Semantics>,
    task_state: Res<TaskState>,
    playback: Res<SemanticPlaybackQueue>,
    mut report_state: ResMut<ReportState>,
) {
    if report_state.printed || task_state.task.is_some() || !playback.is_empty() {
        return;
    }

    report_state.printed = true;

    if let Err(error) = report_task_result_impl(&semantics) {
        panic!("failed to report task result: {error}");
    }
}

fn report_task_result_impl(semantics: &Semantics) -> Result<(), SemanticError> {
    let names = |kinds: Vec<Kind>| -> Vec<String> {
        kinds
            .into_iter()
            .map(|kind| semantics.name(kind).unwrap_or("<unknown>").to_owned())
            .collect()
    };

    let sq_predators = semantics.targets(RABBIT, PREDATED_BY);
    let sq_droppers = semantics.targets(PELT, DROPPED_BY);

    println!("rabbit predated_by: {:?}", names(sq_predators));
    println!("pelt dropped_by: {:?}", names(sq_droppers));

    Ok(())
}

fn main() {
    let _ = AsyncComputeTaskPool::get_or_init(TaskPool::new);

    let mut app = App::new();
    app.init_resource::<TaskState>();
    app.init_resource::<ReportState>();
    app.add_plugins(SemanticsPlugin);
    app.add_systems(Startup, setup_semantics);
    app.add_systems(Update, collect_task_result);
    app.add_systems(Last, (playback_task_result, report_task_result).chain());

    for _ in 0..16 {
        app.update();

        let world = app.world_mut();
        let done = world
            .get_resource::<TaskState>()
            .map(|state| state.task.is_none())
            .unwrap_or(false)
            && world
                .get_resource::<SemanticPlaybackQueue>()
                .map(|queue| queue.is_empty())
                .unwrap_or(false)
            && world
                .get_resource::<ReportState>()
                .map(|state| state.printed)
                .unwrap_or(false);

        if done {
            break;
        }
    }
}
