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

use lycan_serialize::Direction;
use instance::{
    SEC_PER_UPDATE,
    TickEvent,
};
use scripts::AaribaScripts;

mod attacks;
mod movement;

/// Triggers all temporal effects
pub fn update(
    entities: &mut EntityStore,
    notifications: &mut Vec<Notification>,
    scripts: &AaribaScripts,
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
    movement::resolve_movements(entities, notifications);
    attacks::resolve_attacks(entities, notifications, scripts, &mut tick_events);
    generate_position_updates(entities, notifications);
    tick_events
}

fn generate_position_updates(
    entities: &EntityStore,
    notifications: &mut Vec<Notification>,
    ) {
    for entity in entities.iter() {
        let notif = Notification::position(
            entity.get_id().as_u64(),
            entity.position,
            entity.speed,
            entity.pv,
            );
        notifications.push(notif);
    }
}

