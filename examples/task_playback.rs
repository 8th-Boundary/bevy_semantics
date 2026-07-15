use bevy_app::{App, Last, Startup, Update};
use bevy_ecs::prelude::{Res, ResMut, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_semantics::core::{INVERSE_OF, IS_A, RELATION};
use bevy_semantics::prelude::*;
use bevy_tasks::{futures_lite::future, AsyncComputeTaskPool, Task, TaskPool};

// These IDs are available at compile time and can be shared by systems, tasks,
// and semantic components without consulting a runtime registry.
const CREATURE: Kind = kind!("Creature");
const BEAST: Kind = kind!("Beast");
const CANINE: Kind = kind!("Canine");
const WOLF: Kind = kind!("Wolf");
const RABBIT: Kind = kind!("Rabbit");
const ITEM: Kind = kind!("Item");
const RESOURCE: Kind = kind!("Resource");
const HERB: Kind = kind!("Herb");
const LOOT: Kind = kind!("Loot");
const PELT: Kind = kind!("Pelt");
const PLACE: Kind = kind!("Place");
const FOREST: Kind = kind!("Forest");
const PREYS_ON: Kind = kind!("preys_on");
const PREDATED_BY: Kind = kind!("predated_by");
const DROPS: Kind = kind!("drops");
const DROPPED_BY: Kind = kind!("dropped_by");
const GROWS_IN: Kind = kind!("grows_in");

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
    // Registration publishes the names and graph nodes at runtime. Each result
    // must match the compile-time identity declared above.
    assert_eq!(semantics.register_kind("Creature")?, CREATURE);
    assert_eq!(semantics.register_kind("Beast")?, BEAST);
    assert_eq!(semantics.register_kind("Canine")?, CANINE);
    assert_eq!(semantics.register_kind("Wolf")?, WOLF);
    assert_eq!(semantics.register_kind("Rabbit")?, RABBIT);
    assert_eq!(semantics.register_kind("Item")?, ITEM);
    assert_eq!(semantics.register_kind("Resource")?, RESOURCE);
    assert_eq!(semantics.register_kind("Herb")?, HERB);
    assert_eq!(semantics.register_kind("Loot")?, LOOT);
    assert_eq!(semantics.register_kind("Pelt")?, PELT);
    assert_eq!(semantics.register_kind("Place")?, PLACE);
    assert_eq!(semantics.register_kind("Forest")?, FOREST);
    assert_eq!(semantics.register_kind("preys_on")?, PREYS_ON);
    assert_eq!(semantics.register_kind("predated_by")?, PREDATED_BY);
    assert_eq!(semantics.register_kind("drops")?, DROPS);
    assert_eq!(semantics.register_kind("dropped_by")?, DROPPED_BY);
    assert_eq!(semantics.register_kind("grows_in")?, GROWS_IN);

    semantics.add_edge(PREYS_ON, IS_A, RELATION)?;
    semantics.add_edge(PREDATED_BY, IS_A, RELATION)?;
    semantics.add_edge(DROPS, IS_A, RELATION)?;
    semantics.add_edge(DROPPED_BY, IS_A, RELATION)?;
    semantics.add_edge(GROWS_IN, IS_A, RELATION)?;

    semantics.add_edge(PREYS_ON, INVERSE_OF, PREDATED_BY)?;
    semantics.add_edge(PREDATED_BY, INVERSE_OF, PREYS_ON)?;
    semantics.add_edge(DROPS, INVERSE_OF, DROPPED_BY)?;
    semantics.add_edge(DROPPED_BY, INVERSE_OF, DROPS)?;

    semantics.add_edge(BEAST, IS_A, CREATURE)?;
    semantics.add_edge(CANINE, IS_A, BEAST)?;
    semantics.add_edge(WOLF, IS_A, CANINE)?;
    semantics.add_edge(RABBIT, IS_A, BEAST)?;
    semantics.add_edge(RESOURCE, IS_A, ITEM)?;
    semantics.add_edge(HERB, IS_A, RESOURCE)?;
    semantics.add_edge(LOOT, IS_A, ITEM)?;
    semantics.add_edge(PELT, IS_A, LOOT)?;
    semantics.add_edge(FOREST, IS_A, PLACE)?;

    semantics.add_edge(WOLF, PREYS_ON, RABBIT)?;
    semantics.add_edge(WOLF, DROPS, PELT)?;
    semantics.add_edge(HERB, GROWS_IN, FOREST)?;

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

        let mut commands = Vec::with_capacity(sq_prey_edges.len() + sq_drop_edges.len());
        commands.extend(
            sq_prey_edges
                .into_iter()
                .map(|edge| SemanticCommand::AddEdge {
                    subject: edge.target,
                    relation: PREDATED_BY,
                    target: edge.subject,
                    weight: None,
                }),
        );
        commands.extend(
            sq_drop_edges
                .into_iter()
                .map(|edge| SemanticCommand::AddEdge {
                    subject: edge.target,
                    relation: DROPPED_BY,
                    target: edge.subject,
                    weight: None,
                }),
        );
        commands
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
