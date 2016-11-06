use lycan_serialize::Direction;
use aariba::expressions::{Store};

use instance::SEC_PER_UPDATE;
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
    ) {
    let mut double_iterator = entities.iter_mut_wrapper();
    while let Some((entity, mut wrapper)) = double_iterator.next_item() {
        trace!("Entity {} {:?}", entity.id, entity.attacking);
        match entity.attacking {
            AttackState::Idle => {}
            AttackState::Attacking => {
                entity.attacking = AttackState::Reloading(1.0);
                resolve_hit(entity, &mut wrapper, notifications, scripts);
            }
            AttackState::Reloading(delay) => {
                let remaining = delay - entity.stats.attack_speed * *SEC_PER_UPDATE;
                if remaining < 0.0 {
                    entity.attacking = AttackState::Idle;
                } else {
                    entity.attacking = AttackState::Reloading(remaining);
                }
            }
        }
    }
}

fn resolve_hit(
    attacker: &mut Entity,
    others: &mut OthersAccessor,
    _notifications: &mut Vec<Notification>,
    scripts: &AaribaScripts,
    ) {
    for entity in others.iter_mut() {
        if attack_success(attacker, entity) {
            let mut integration = AaribaIntegration::new(attacker, entity);
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
struct AaribaIntegration<'a,'b> {
    source: &'a mut Entity,
    target: &'b mut Entity,
}

impl <'a,'b> Store for AaribaIntegration<'a,'b> {
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
                set_attribute(self.target, second, value)
            }
            "source" => {
                set_attribute(self.source, second, value)
            }
            _ => Err(()),
        }
    }
}

fn set_attribute(
    entity: &mut Entity,
    var: &str,
    value: f64,
    ) -> Result<Option<f64>,()> {
    match var {
        "pv" => {
            let old = entity.pv as f64;
            entity.pv = value as u64;
            Ok(Some(old))
        }
        _ => Err(()),
    }
}

impl <'a,'b> AaribaIntegration<'a,'b> {
    fn new(source: &'a mut Entity, target: &'b mut Entity) -> AaribaIntegration<'a, 'b> {
        AaribaIntegration {
            source: source,
            target: target,
        }
    }
}
