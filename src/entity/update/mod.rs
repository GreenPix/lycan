// Intended to be the part that handles all the collision / effects and other core
// features of the game engine

use std::collections::HashMap;

use entity::{
    Entity,
    Order,
    EntityStore,
    OthersAccessor,
};
use messages::Notification;
use id::Id;
use data::Map;

use lycan_serialize::Direction;
use instance::{
    TickEvent,
};
use scripts::AaribaScripts;

mod attacks;
mod movement;

/// Triggers all temporal effects
pub fn update(
    entities: &mut EntityStore,
    notifications: &mut Vec<Notification>,
    map: &Map,
    scripts: &AaribaScripts,
    tick_id: u64,
    tick_duration: f32,
    ) -> Vec<TickEvent> {
    // During a tick, every event that can affect an entity (an entity attacking, a spell cast,
    // a projectile hitting) will be randomly ordered, and all of them will be executed in
    // sequence.
    // In other words, if two actions happen during the same server tick, the order between those
    // two actions will be *random*, but there will be one: the result will be the same as if the
    // two actions happened during two separate ticks.
    // This algorithm aims to prevent bad interractions between spells, leading to weird behaviour
    // when happening during the same tick

    let mut tick_events = Vec::new();
    movement::resolve_movements(entities, notifications, map, tick_duration);
    attacks::resolve_attacks(entities, notifications, scripts, &mut tick_events, tick_duration);
    generate_position_updates(entities, notifications, tick_id);
    tick_events
}

fn generate_position_updates(
    entities: &EntityStore,
    notifications: &mut Vec<Notification>,
    tick_id: u64,
    ) {
    let entities_updates = entities.iter()
        .map(|e| e.to_entity_update())
        .collect();
    let notif = Notification::GameUpdate {
        tick_id: tick_id,
        entities: entities_updates,
    };
    notifications.push(notif);
}

