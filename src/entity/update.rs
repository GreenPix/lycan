// Intended to be the part that handles all the collision / effects and other core
// features of the game engine

use std::collections::HashMap;

use super::{Entity,Order,EntityStore,Wrapper};
use messages::Notification;
use id::Id;
use nalgebra::Vec2;
use nalgebra::Pnt2;
use aariba::expressions::{Store};

use lycan_serialize::Direction;
use instance::SEC_PER_UPDATE;
use scripts::AaribaScripts;

// Reason why an action has been rejected
// TODO: Put in lycan-serialize
pub enum Error {
    AlreadyAttacking,
}


/// Triggers all temporal effects
pub fn update(
    entities: &mut EntityStore,
    notifications: &mut Vec<Notification>,
    scripts: &AaribaScripts,
    ) {
    {
        let mut double_iterator = entities.iter_mut_wrapper();
        while let Some((entity, mut wrapper)) = double_iterator.next_item() {
            entity.trigger_personal_effects(notifications);
            entity.check_collisions(&mut wrapper, scripts);
        }
    }
    generate_position_updates(entities, notifications);
}

fn generate_position_updates(
    entities: &EntityStore,
    notifications: &mut Vec<Notification>,
    ) {
    for entity in entities.iter() {
        let notif = Notification::position(
            entity.get_id().as_u64(),
            entity.data.position,
            entity.data.speed,
            entity.data.pv,
            );
        notifications.push(notif);
    }
}

impl Entity {
    /// Apply an order to an entity, and optionally returns a notification
    pub fn apply(&mut self, order: Order) -> Result<Option<Notification>,Error> {
        debug!("Received order {:?}", order);
        match order {
            Order::Walk(orientation) => {
                match orientation {
                    None => self.data.walking = false,
                    Some(o) => {
                        self.data.orientation = o;
                        self.data.walking = true;
                    }
                }
                Ok(Some(Notification::walk(self.id.as_u64(), orientation)))
            }
            Order::Say(message) => {
                Ok(Some(Notification::say(self.id.as_u64(), message)))
            }
            Order::Attack => {
                // If the entity was already in the middle of an attack, ignore
                if self.data.attacking == 0 {
                    self.data.attacking = 60;
 
                    // TODO: Attacking notification
                    Ok(None)
                } else {
                    Err(Error::AlreadyAttacking)
                }
            }
        }
    }

    /// Triggers everything that does not interract with other entities
    ///
    /// For example, we trigger updates of long lasting spells
    fn trigger_personal_effects(&mut self, _notifications: &mut Vec<Notification>) {
        if self.data.attacking > 0 {
            self.data.attacking -= 1;
        }
    }

    /// Checks for collisions with others (attack, movement ...)
    fn check_collisions(&mut self,
                        others: &mut Wrapper,
                        scripts: &AaribaScripts,
                        ) {
        // TODO: Broad phase first?

        if self.data.attacking == 30 {
            for entity in others.iter() {
                if attack_success(self, entity) {
                    let mut integration = AaribaIntegration::new(entity);
                    match scripts.combat.evaluate(&mut integration) {
                        Ok(()) => {}
                        Err(e) => {
                            error!("Script error: {:#?}", e);
                            continue;
                        }
                    }
                }
            }
        }

        // Assume no collisions at the moment ...
        let unitary_speed = if self.data.walking {
            match self.data.orientation {
                Direction::North => Vec2::new(0.0, 1.0),
                Direction::South => Vec2::new(0.0, -1.0),
                Direction::East  => Vec2::new(1.0, 0.0),
                Direction::West  => Vec2::new(-1.0, 0.0),
            }
        } else {
            Vec2::new(0.0, 0.0)
        };
        let speed = unitary_speed * self.data.stats.speed;
        let new_position = self.data.position + (speed * *SEC_PER_UPDATE);
        self.data.position = new_position;
        self.data.speed = speed;
    }
}

fn attack_success(attacker: &Entity, target: &Entity) -> bool {
    let target_box = target.data.hitbox;
    let target_position = target.data.position;
    let attack_box;
    let attack_position;
    match attacker.get_orientation() {
        Direction::North => {
            attack_box = attacker.data.attack_box.rotated();
            attack_position = attacker.data.position + attacker.data.attack_offset_y;
        }
        Direction::South => {
            attack_box = attacker.data.attack_box.rotated();
            attack_position = attacker.data.position - attacker.data.attack_offset_y;
        }
        Direction::East => {
            attack_box = attacker.data.attack_box;
            attack_position = attacker.data.position + attacker.data.attack_offset_x;
        }
        Direction::West => {
            attack_box = attacker.data.attack_box;
            attack_position = attacker.data.position - attacker.data.attack_offset_x;
        }
    }

    attack_box.collision(attack_position, &target_box, target_position)
}

// Does it still make sense to have a separate type?
// We could just implement Store for Entity
#[derive(Debug)]
struct AaribaIntegration<'a> {
    target: &'a mut Entity,
}

impl <'a> Store for AaribaIntegration<'a> {
    fn get_attribute(&self, var: &str) -> Option<f64> {
        match var {
            "pv" => Some(self.target.data.pv as f64),
            _ => None,
        }
    }
    fn set_attribute(&mut self, var: &str, value: f64) -> Result<Option<f64>,()> {
        match var {
            "pv" => {
                let old = self.target.data.pv as f64;
                self.target.data.pv = value as u64;
                Ok(Some(old))
            }
            _ => Err(()),
        }
    }
}

impl <'a> AaribaIntegration<'a> {
    fn new(entity: &'a mut Entity) -> AaribaIntegration<'a> {
        AaribaIntegration {
            target: entity,
        }
    }
}
