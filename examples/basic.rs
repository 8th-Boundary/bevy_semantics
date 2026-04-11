use bevy_app::{App, Last, Startup};
use bevy_ecs::prelude::{Res, ResMut};
use bevy_semantics::prelude::{EdgeDirection, Semantics, SemanticsPlugin};

fn setup_semantics(mut semantics: ResMut<Semantics>) {
    // Bevy system functions stay simple here; the helper carries the fallible setup logic.
    if let Err(error) = setup_semantics_impl(&mut semantics) {
        panic!("failed to seed semantics: {error}");
    }
}

fn setup_semantics_impl(semantics: &mut Semantics) -> Result<(), bevy_semantics::SemanticError> {
    let builtins = semantics.ensure_builtins()?;

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

    semantics.add_edge(preys_on, builtins.is_a, builtins.relation)?;
    semantics.add_edge(predated_by, builtins.is_a, builtins.relation)?;
    semantics.add_edge(drops, builtins.is_a, builtins.relation)?;
    semantics.add_edge(dropped_by, builtins.is_a, builtins.relation)?;
    semantics.add_edge(grows_in, builtins.is_a, builtins.relation)?;

    semantics.add_edge(preys_on, builtins.inverse_of, predated_by)?;
    semantics.add_edge(predated_by, builtins.inverse_of, preys_on)?;
    semantics.add_edge(drops, builtins.inverse_of, dropped_by)?;
    semantics.add_edge(dropped_by, builtins.inverse_of, drops)?;

    semantics.add_edge(beast, builtins.is_a, creature)?;
    semantics.add_edge(canine, builtins.is_a, beast)?;
    semantics.add_edge(wolf, builtins.is_a, canine)?;
    semantics.add_edge(rabbit, builtins.is_a, beast)?;
    semantics.add_edge(resource, builtins.is_a, item)?;
    semantics.add_edge(herb, builtins.is_a, resource)?;
    semantics.add_edge(loot, builtins.is_a, item)?;
    semantics.add_edge(pelt, builtins.is_a, loot)?;
    semantics.add_edge(forest, builtins.is_a, place)?;

    semantics.add_edge(wolf, preys_on, rabbit)?;
    semantics.add_edge(rabbit, predated_by, wolf)?;
    semantics.add_edge(wolf, drops, pelt)?;
    semantics.add_edge(pelt, dropped_by, wolf)?;
    semantics.add_edge(herb, grows_in, forest)?;

    Ok(())
}

fn inspect_semantics(semantics: Res<Semantics>) {
    if let Err(error) = inspect_semantics_impl(&semantics) {
        panic!("failed to inspect semantics: {error}");
    }
}

fn inspect_semantics_impl(semantics: &Semantics) -> Result<(), bevy_semantics::SemanticError> {
    let builtins = semantics.builtins();
    let is_a = builtins.is_a;
    let preys_on = semantics.kind("preys_on")?;
    let predated_by = semantics.kind("predated_by")?;
    let drops = semantics.kind("drops")?;
    let dropped_by = semantics.kind("dropped_by")?;
    let grows_in = semantics.kind("grows_in")?;
    let wolf = semantics.kind("Wolf")?;
    let rabbit = semantics.kind("Rabbit")?;
    let herb = semantics.kind("Herb")?;
    let pelt = semantics.kind("Pelt")?;
    let forest = semantics.kind("Forest")?;
    let creature = semantics.kind("Creature")?;

    let names = |kinds: Vec<bevy_semantics::Kind>| -> Vec<String> {
        kinds
            .into_iter()
            .map(|kind| semantics.name(kind).unwrap_or("<unknown>").to_owned())
            .collect()
    };

    // `sq_` marks semantic query results, which are derived from semantic reads.
    let mut sq_wolf_lineage = vec![wolf];
    sq_wolf_lineage.extend(semantics.reachable(wolf, is_a, EdgeDirection::Outgoing, usize::MAX));
    let sq_wolf_prey = semantics.targets(wolf, preys_on);
    let sq_rabbit_predators = semantics.targets(rabbit, predated_by);
    let sq_wolf_drops = semantics.targets(wolf, drops);
    let sq_pelt_dropped_by = semantics.targets(pelt, dropped_by);
    let sq_herb_sites = semantics.targets(herb, grows_in);

    println!("wolf lineage: {:?}", names(sq_wolf_lineage));
    println!("wolf preys_on: {:?}", names(sq_wolf_prey));
    println!("rabbit predated_by: {:?}", names(sq_rabbit_predators));
    println!("wolf drops: {:?}", names(sq_wolf_drops));
    println!("pelt dropped_by: {:?}", names(sq_pelt_dropped_by));
    println!("herb grows_in: {:?}", names(sq_herb_sites));
    println!("wolf is_a creature: {}", semantics.is_a(wolf, creature));
    println!("pelt kind present: {}", semantics.name(pelt).is_some());
    println!("forest kind present: {}", semantics.name(forest).is_some());

    Ok(())
}

fn main() {
    let mut app = App::new();
    app.add_plugins(SemanticsPlugin);
    app.add_systems(Startup, setup_semantics);
    app.add_systems(Last, inspect_semantics);
    app.update();
}
