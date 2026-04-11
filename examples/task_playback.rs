use bevy_app::{App, Last, Startup, Update};
use bevy_ecs::prelude::{Res, ResMut, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_semantics::prelude::*;
use bevy_tasks::{futures_lite::future, AsyncComputeTaskPool, Task, TaskPool};

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
    let core = semantics.core();

    let creature = semantics.register_kind("Creature")?;
    let beast = semantics.register_kind("Beast")?;
    let canine = semantics.register_kind("Canine")?;
    let wolf = semantics.register_kind("Wolf")?;
    let rabbit = semantics.register_kind("Rabbit")?;
    let item = semantics.register_kind("Item")?;
    let resource = semantics.register_kind("Resource")?;
    let herb = semantics.register_kind("Herb")?;
    let loot = semantics.register_kind("Loot")?;
    let pelt = semantics.register_kind("Pelt")?;
    let place = semantics.register_kind("Place")?;
    let forest = semantics.register_kind("Forest")?;
    let preys_on = semantics.register_kind("preys_on")?;
    let predated_by = semantics.register_kind("predated_by")?;
    let drops = semantics.register_kind("drops")?;
    let dropped_by = semantics.register_kind("dropped_by")?;
    let grows_in = semantics.register_kind("grows_in")?;

    semantics.add_edge(preys_on, core.is_a, core.relation)?;
    semantics.add_edge(predated_by, core.is_a, core.relation)?;
    semantics.add_edge(drops, core.is_a, core.relation)?;
    semantics.add_edge(dropped_by, core.is_a, core.relation)?;
    semantics.add_edge(grows_in, core.is_a, core.relation)?;

    semantics.add_edge(preys_on, core.inverse_of, predated_by)?;
    semantics.add_edge(predated_by, core.inverse_of, preys_on)?;
    semantics.add_edge(drops, core.inverse_of, dropped_by)?;
    semantics.add_edge(dropped_by, core.inverse_of, drops)?;

    semantics.add_edge(beast, core.is_a, creature)?;
    semantics.add_edge(canine, core.is_a, beast)?;
    semantics.add_edge(wolf, core.is_a, canine)?;
    semantics.add_edge(rabbit, core.is_a, beast)?;
    semantics.add_edge(resource, core.is_a, item)?;
    semantics.add_edge(herb, core.is_a, resource)?;
    semantics.add_edge(loot, core.is_a, item)?;
    semantics.add_edge(pelt, core.is_a, loot)?;
    semantics.add_edge(forest, core.is_a, place)?;

    semantics.add_edge(wolf, preys_on, rabbit)?;
    semantics.add_edge(wolf, drops, pelt)?;
    semantics.add_edge(herb, grows_in, forest)?;

    let snapshot = semantics.snapshot();
    task_state.task = Some(AsyncComputeTaskPool::get().spawn(async move {
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

        let mut commands = Vec::with_capacity(sq_prey_edges.len() + sq_drop_edges.len());
        commands.extend(
            sq_prey_edges
                .into_iter()
                .map(|edge| SemanticCommand::AddEdge {
                    subject: edge.target,
                    relation: predated_by,
                    target: edge.subject,
                    weight: None,
                }),
        );
        commands.extend(
            sq_drop_edges
                .into_iter()
                .map(|edge| SemanticCommand::AddEdge {
                    subject: edge.target,
                    relation: dropped_by,
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
    let rabbit = semantics.kind("Rabbit")?;
    let pelt = semantics.kind("Pelt")?;
    let predated_by = semantics.kind("predated_by")?;
    let dropped_by = semantics.kind("dropped_by")?;

    let names = |kinds: Vec<Kind>| -> Vec<String> {
        kinds
            .into_iter()
            .map(|kind| semantics.name(kind).unwrap_or("<unknown>").to_owned())
            .collect()
    };

    let sq_predators = semantics.targets(rabbit, predated_by);
    let sq_droppers = semantics.targets(pelt, dropped_by);

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
