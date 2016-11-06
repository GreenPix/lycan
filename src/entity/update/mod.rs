// Intended to be the part that handles all the collision / effects and other core
// features of the game engine

use std::collections::HashMap;

use super::{Entity,Order,EntityStore,OthersAccessor};
use messages::Notification;
use id::Id;

use lycan_serialize::Direction;
use instance::SEC_PER_UPDATE;
use scripts::AaribaScripts;

mod attacks;
mod movement;

/// Triggers all temporal effects
pub fn update(
    entities: &mut EntityStore,
    notifications: &mut Vec<Notification>,
    scripts: &AaribaScripts,
    ) {
    attacks::resolve_attacks(entities, notifications, scripts);
    movement::resolve_movements(entities, notifications);
    generate_position_updates(entities, notifications);
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

