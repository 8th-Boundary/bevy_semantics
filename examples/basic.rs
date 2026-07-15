use bevy_app::{App, Last, Startup};
use bevy_ecs::prelude::{Res, ResMut};
use bevy_semantics::core::{INVERSE_OF, IS_A, RELATION};
use bevy_semantics::prelude::{semantic_kinds, EdgeDirection, Semantics, SemanticsPlugin};

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

fn setup_semantics(mut semantics: ResMut<Semantics>) {
    // Bevy system functions stay simple here; the helper carries the fallible setup logic.
    if let Err(error) = setup_semantics_impl(&mut semantics) {
        panic!("failed to seed semantics: {error}");
    }
}

fn setup_semantics_impl(semantics: &mut Semantics) -> Result<(), bevy_semantics::SemanticError> {
    semantics.register_consts(DEMO_KINDS)?;

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
    semantics.add_edge(RABBIT, PREDATED_BY, WOLF)?;
    semantics.add_edge(WOLF, DROPS, PELT)?;
    semantics.add_edge(PELT, DROPPED_BY, WOLF)?;
    semantics.add_edge(HERB, GROWS_IN, FOREST)?;

    Ok(())
}

fn inspect_semantics(semantics: Res<Semantics>) {
    if let Err(error) = inspect_semantics_impl(&semantics) {
        panic!("failed to inspect semantics: {error}");
    }
}

fn inspect_semantics_impl(semantics: &Semantics) -> Result<(), bevy_semantics::SemanticError> {
    // Runtime lookup remains useful when a name comes from authored/dynamic data.
    let creature = semantics.kind("Creature")?;

    let names = |kinds: Vec<bevy_semantics::Kind>| -> Vec<String> {
        kinds
            .into_iter()
            .map(|kind| semantics.name(kind).unwrap_or("<unknown>").to_owned())
            .collect()
    };

    // `sq_` marks semantic query results, which are derived from semantic reads.
    let mut sq_wolf_lineage = vec![WOLF];
    sq_wolf_lineage.extend(semantics.reachable(WOLF, IS_A, EdgeDirection::Outgoing, usize::MAX));
    let sq_wolf_prey = semantics.targets(WOLF, PREYS_ON);
    let sq_rabbit_predators = semantics.targets(RABBIT, PREDATED_BY);
    let sq_wolf_drops = semantics.targets(WOLF, DROPS);
    let sq_pelt_dropped_by = semantics.targets(PELT, DROPPED_BY);
    let sq_herb_sites = semantics.targets(HERB, GROWS_IN);

    println!("wolf lineage: {:?}", names(sq_wolf_lineage));
    println!("wolf preys_on: {:?}", names(sq_wolf_prey));
    println!("rabbit predated_by: {:?}", names(sq_rabbit_predators));
    println!("wolf drops: {:?}", names(sq_wolf_drops));
    println!("pelt dropped_by: {:?}", names(sq_pelt_dropped_by));
    println!("herb grows_in: {:?}", names(sq_herb_sites));
    println!("wolf is_a creature: {}", semantics.is_a(WOLF, creature));
    println!("pelt kind present: {}", semantics.name(PELT).is_some());
    println!("forest kind present: {}", semantics.name(FOREST).is_some());

    Ok(())
}

fn main() {
    let mut app = App::new();
    app.add_plugins(SemanticsPlugin);
    app.add_systems(Startup, setup_semantics);
    app.add_systems(Last, inspect_semantics);
    app.update();
}
