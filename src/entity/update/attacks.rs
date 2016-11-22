use lycan_serialize::Direction;
use aariba::expressions::{Store};

use id::Id;
use instance::{
    TickEvent,
};
use entity::{
    Entity,
    Order,
    EntityStore,
    OthersAccessor,
    AttackState,
};
use messages::Notification;
use scripts::AaribaScripts;

pub fn resolve_attacks(
    entities: &mut EntityStore,
    notifications: &mut Vec<Notification>,
    scripts: &AaribaScripts,
    events: &mut Vec<TickEvent>,
    tick_duration: f32,
    ) {
    // Indicates entities that die during that tick
    // As soon as the entity dies, it should *stop interracting with the world*
    let mut dead_entities_id = vec![];

    {
        // Iterate through all entities
        let mut double_iterator = entities.iter_mut_wrapper();
        while let Some((entity, mut wrapper)) = double_iterator.next_item() {
            if !dead_entities_id.contains(&entity.id) {
                trace!("Entity {} {:?}", entity.id, entity.attacking);
                match entity.attacking {
                    AttackState::Idle => {}
                    AttackState::Attacking => {
                        entity.attacking = AttackState::Reloading(1.0);
                        resolve_hit(entity, &mut wrapper, notifications, scripts, &mut dead_entities_id);
                    }
                    AttackState::Reloading(delay) => {
                        let remaining = delay - entity.stats.attack_speed * tick_duration;
                        if remaining < 0.0 {
                            entity.attacking = AttackState::Idle;
                        } else {
                            entity.attacking = AttackState::Reloading(remaining);
                        }
                    }
                }
            }
        }
    }

    for dead_id in dead_entities_id {
        match entities.remove(dead_id) {
            Some(dead_entity) => {
                events.push(TickEvent::EntityDeath(dead_entity));
            }
            None => {
                error!("Could not find dead entity {} in the store, but it was scheduled for removal",
                       dead_id);
            }
        }
    }
}

fn resolve_hit(
    attacker: &mut Entity,
    others: &mut OthersAccessor,
    notifications: &mut Vec<Notification>,
    scripts: &AaribaScripts,
    dead_entities_id: &mut Vec<Id<Entity>>,
    ) {
    for entity in others.iter_mut() {
        if !dead_entities_id.contains(&entity.id) {
            if attack_success(attacker, entity) {
                let mut integration = AaribaIntegration::new(attacker,
                                                             entity,
                                                             notifications,
                                                             dead_entities_id,
                                                             );
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
}

fn attack_success(attacker: &Entity, target: &Entity) -> bool {
    let target_box = target.hitbox;
    let target_position = target.position;
    let attack_box;
    let attack_position;
    match attacker.get_orientation() {
        Direction::North => {
            attack_box = attacker.attack_box.rotated();
            attack_position = attacker.position + attacker.attack_offset_y;
        }
        Direction::South => {
            attack_box = attacker.attack_box.rotated();
            attack_position = attacker.position - attacker.attack_offset_y;
        }
        Direction::East => {
            attack_box = attacker.attack_box;
            attack_position = attacker.position + attacker.attack_offset_x;
        }
        Direction::West => {
            attack_box = attacker.attack_box;
            attack_position = attacker.position - attacker.attack_offset_x;
        }
    }

    attack_box.collision(attack_position, &target_box, target_position)
}

#[derive(Debug)]
struct AaribaIntegration<'a,'b, 'c, 'd> {
    source: &'a mut Entity,
    target: &'b mut Entity,
    notifications: &'c mut Vec<Notification>,
    dead_entities_id: &'d mut Vec<Id<Entity>>,
}

impl <'a, 'b, 'c, 'd> Store for AaribaIntegration<'a, 'b, 'c, 'd> {
    fn get_attribute(&self, var: &str) -> Option<f64> {
        let mut splitn = var.splitn(2, '.');
        let first = match splitn.next() {
            Some(first) => first,
            None => return None,
        };
        let second = match splitn.next() {
            Some(s) => s,
            None => return None,
        };
        match first {
            "target" => self.target.get_attribute(second),
            "source" => self.source.get_attribute(second),
            _ => None,
        }
    }
    fn set_attribute(&mut self, var: &str, value: f64) -> Result<Option<f64>,()> {
        let mut splitn = var.splitn(2, '.');
        let first = match splitn.next() {
            Some(first) => first,
            None => return Err(()),
        };
        let second = match splitn.next() {
            Some(s) => s,
            None => return Err(()),
        };
        match first {
            "target" => {
                set_attribute(self.target,
                              self.source.id,
                              second,
                              value,
                              self.notifications,
                              self.dead_entities_id)
            }
            "source" => {
                let id = self.source.id;
                set_attribute(self.source,
                              id,
                              second,
                              value,
                              self.notifications,
                              self.dead_entities_id)
            }
            _ => Err(()),
        }
    }
}

fn set_attribute(
    entity: &mut Entity,
    source: Id<Entity>,     // Can potentially be the same as entity.id
    var: &str,
    value: f64,
    notifications: &mut Vec<Notification>,
    dead_entities_id: &mut Vec<Id<Entity>>,
    ) -> Result<Option<f64>,()> {
    match var {
        "damage" => {
            if entity.pv != 0 {
                notifications.push(Notification::Damage {
                    source: source.as_u64(),
                    victim: entity.id.as_u64(),
                    amount: value as u64,
                });
                let new_pv = entity.pv as f64 - value;
                if new_pv < 0.0 {
                    // Death of entity
                    entity.pv = 0;
                    dead_entities_id.push(entity.id);
                } else {
                    entity.pv = new_pv as u64;
                }
            } else {
                warn!("Trying to damage a dead entity {}", entity.id);
            }
            Ok(None)
        }
        _ => Err(()),
    }
}

impl <'a, 'b, 'c, 'd> AaribaIntegration<'a, 'b, 'c, 'd> {
    fn new(
        source: &'a mut Entity,
        target: &'b mut Entity,
        notifications: &'c mut Vec<Notification>,
        dead_entities_id: &'d mut Vec<Id<Entity>>,
        ) -> AaribaIntegration<'a, 'b, 'c, 'd> {
        AaribaIntegration {
            source: source,
            target: target,
            notifications: notifications,
            dead_entities_id: dead_entities_id,
        }
    }
}
