use nalgebra::Vector2;
use nalgebra::Point2;

use lycan_serialize::Direction;

use messages::Notification;
use instance::SEC_PER_UPDATE;
use entity::{
    Entity,
    Order,
    EntityStore,
};

pub fn resolve_movements(
    entities: &mut EntityStore,
    notifications: &mut Vec<Notification>,
    ) {
    for entity in entities.iter_mut() {
        resolve_collisions(entity, notifications)
    }
}

fn resolve_collisions(
    entity: &mut Entity,
    _notifications: &mut Vec<Notification>,
    ) {
    // Assume no collisions at the moment ...
    let unitary_speed = if entity.walking {
        match entity.orientation {
            Direction::North => Vector2::new(0.0, 1.0),
            Direction::South => Vector2::new(0.0, -1.0),
            Direction::East  => Vector2::new(1.0, 0.0),
            Direction::West  => Vector2::new(-1.0, 0.0),
        }
    } else {
        Vector2::new(0.0, 0.0)
    };
    let speed = unitary_speed * entity.stats.speed;
    let new_position = entity.position + (speed * *SEC_PER_UPDATE);
    entity.position = new_position;
    entity.speed = speed;
}
