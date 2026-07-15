use bevy_app::{App, Last, Startup};
use bevy_ecs::prelude::{Res, ResMut};
use bevy_semantics::prelude::{EdgeDirection, Semantics, SemanticsPlugin};

// `kind!` creates the stable ID at compile time. `register_kind` separately
// publishes its canonical name into this runtime registry for authored graphs.
macro_rules! register_static_kind {
    ($semantics:expr, $name:literal) => {{
        let registered = $semantics.register_kind($name)?;
        assert_eq!(registered, bevy_semantics::kind!($name));
        registered
    }};
}

fn setup_semantics(mut semantics: ResMut<Semantics>) {
    // Bevy system functions stay simple here; the helper carries the fallible setup logic.
    if let Err(error) = setup_semantics_impl(&mut semantics) {
        panic!("failed to seed semantics: {error}");
    }
}

fn setup_semantics_impl(semantics: &mut Semantics) -> Result<(), bevy_semantics::SemanticError> {
    let core = semantics.core();

    let creature = register_static_kind!(semantics, "Creature");
    let beast = register_static_kind!(semantics, "Beast");
    let canine = register_static_kind!(semantics, "Canine");
    let wolf = register_static_kind!(semantics, "Wolf");
    let rabbit = register_static_kind!(semantics, "Rabbit");
    let item = register_static_kind!(semantics, "Item");
    let resource = register_static_kind!(semantics, "Resource");
    let herb = register_static_kind!(semantics, "Herb");
    let loot = register_static_kind!(semantics, "Loot");
    let pelt = register_static_kind!(semantics, "Pelt");
    let place = register_static_kind!(semantics, "Place");
    let forest = register_static_kind!(semantics, "Forest");
    let preys_on = register_static_kind!(semantics, "preys_on");
    let predated_by = register_static_kind!(semantics, "predated_by");
    let drops = register_static_kind!(semantics, "drops");
    let dropped_by = register_static_kind!(semantics, "dropped_by");
    let grows_in = register_static_kind!(semantics, "grows_in");

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
    let core = semantics.core();
    let is_a = core.is_a;
    let preys_on = bevy_semantics::kind!("preys_on");
    let predated_by = bevy_semantics::kind!("predated_by");
    let drops = bevy_semantics::kind!("drops");
    let dropped_by = bevy_semantics::kind!("dropped_by");
    let grows_in = bevy_semantics::kind!("grows_in");
    let wolf = bevy_semantics::kind!("Wolf");
    let rabbit = bevy_semantics::kind!("Rabbit");
    let herb = bevy_semantics::kind!("Herb");
    let pelt = bevy_semantics::kind!("Pelt");
    let forest = bevy_semantics::kind!("Forest");
    // Runtime lookup remains useful when a name comes from authored/dynamic data.
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
