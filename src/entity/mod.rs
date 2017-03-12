use std::fmt::{self,Formatter};
use std::cell::{RefCell,RefMut,Ref};
use std::sync::atomic::{AtomicUsize, Ordering};

use nalgebra::{Point2,Vector2};
use rand;

use id::{Id,HasForgeableId,HasId};
use data::{
    Map,Player,Stats,Position,
    EntityManagement, PositionInstance,
    PlayerStruct, MonsterStruct,
    EntityType as DataEntityType,
    Monster,
};
use messages::{
    EntityState,
    Notification,
    EntityUpdate,
};
use instance::Instance;
use actor::ActorId;

use self::hitbox::RectangleHitbox;
pub use self::double_iterator::{DoubleIterMut,OthersAccessor,OthersIter,OthersIterMut};
pub use self::store::EntityStore;

mod status;
mod update;
mod hitbox;
mod double_iterator;
mod store;
//mod serialize;

pub use self::update::update;
pub use lycan_serialize::Order;

pub use lycan_serialize::Direction;

static DEFAULT_SPEED:    f32 = 10.0;
static DEFAULT_AI_SPEED: f32 = 5.0;
static DEFAULT_ATTACK_SPEED: f32 = 2.0; // 2 attacks per seconds

#[derive(Debug)]
pub struct Entity {
    id: Id<Entity>,

    actor: Option<ActorId>,
    e_type: EntityType,
    position: Point2<f32>,
    // We probably won't save the speed ...
    speed: Vector2<f32>,
    orientation: Direction,
    skin: u64,
    pv: u64,
    hitbox: RectangleHitbox,
    attack_box: RectangleHitbox,
    attack_offset_x: Vector2<f32>,
    attack_offset_y: Vector2<f32>,
    base_stats: Stats,
    stats: CurrentStats,

    // TODO: Replace by a FSM
    walking: bool,
    attacking: AttackState,
}

impl Entity {
    pub fn new(e_type: EntityType,
               position: Point2<f32>,
               orientation: Direction,
               skin: u64,
               base_stats: Stats,
               pv: u64,
               )
        -> Entity {
            let mut e = Entity {
                id: Id::new(),

                actor: None,
                e_type: e_type,
                position: position,
                speed: Vector2::new(0.0,0.0),
                orientation: orientation,
                base_stats: base_stats,
                stats: Default::default(),
                skin: skin,
                pv: pv,
                hitbox: RectangleHitbox::new_default(),
                attack_box: RectangleHitbox::new(0.5, 0.5),
                attack_offset_x: Vector2::new(0.75, 0.0),
                attack_offset_y: Vector2::new(0.0, 1.0),

                walking: false,
                attacking: AttackState::Idle,
            };
            e.recompute_current_stats();
            e
        }

    pub fn is_player(&self) -> bool {
        if let EntityType::Player(_) = self.e_type {
            true
        } else {
            false
        }
    }

    pub fn is_monster(&self) -> bool {
        if let EntityType::Monster(_) = self.e_type {
            true
        } else {
            false
        }
    }

    // Takes effects into account
    pub fn recompute_current_stats(&mut self) {
        self.stats.speed = self.get_nominal_speed();
        self.stats.strength = self.base_stats.strength;
        self.stats.dexterity = self.base_stats.dexterity;
        self.stats.constitution = self.base_stats.constitution;
        self.stats.intelligence = self.base_stats.intelligence;
        self.stats.precision = self.base_stats.precision;
        self.stats.wisdom = self.base_stats.wisdom;
        self.stats.attack_speed = DEFAULT_ATTACK_SPEED;
    }

    fn get_attribute(&self, var: &str) -> Option<f64> {
        match var {
            "pv" => Some(self.pv as f64),
            "strength" => Some(self.stats.strength as f64),
            "dexterity" => Some(self.stats.dexterity as f64),
            "constitution" => Some(self.stats.constitution as f64),
            "intelligence" => Some(self.stats.intelligence as f64),
            "precision" => Some(self.stats.precision as f64),
            "wisdom" => Some(self.stats.wisdom as f64),
            "speed" => Some(self.stats.speed as f64),
            _ => None,
        }
    }

    pub fn walk(&mut self, orientation: Option<Direction>) {
        match orientation {
            Some(o) => {
                self.walking = true;
                self.orientation = o;
            }
            None => {
                self.walking = false;
            }
        }
    }

    /// Apply an order to an entity, and optionally returns a notification
    pub fn apply(&mut self, order: Order) -> Result<Option<Notification>,Error> {
        debug!("Received order {:?}", order);
        match order {
            Order::Walk(orientation) => {
                match orientation {
                    None => self.walking = false,
                    Some(o) => {
                        self.orientation = o;
                        self.walking = true;
                    }
                }
                Ok(Some(Notification::walk(self.id.as_u64(), orientation)))
            }
            Order::Say(message) => {
                Ok(Some(Notification::say(self.id.as_u64(), message)))
            }
            Order::Attack => {
                match self.attacking {
                    AttackState::Idle => {
                        self.attacking = AttackState::Attacking;
                        // TODO: Attacking notification
                        Ok(None)
                    }
                    // If the entity was already in the middle of an attack, ignore
                    AttackState::Attacking => { Err(Error::AlreadyAttacking) }
                    AttackState::Reloading(_) => { Err(Error::AlreadyAttacking) }
                }
            }
        }
    }

    pub fn get_map_position(&self) -> Option<Id<Map>> {
        match self.e_type {
            EntityType::Player(ref player) => Some(player.map),
            _ => None,
        }
    }

    pub fn dump(&self, f: &mut Formatter, indent: &str) -> Result<(),fmt::Error> {
        try!(writeln!(f, "{}Entity {}", indent, self.id));
        match self.e_type {
            EntityType::Player(ref player) => {
                try!(writeln!(f, "{}Player {} {} attached to map {}",
                              indent,
                              player.id,
                              &player.name,
                              player.map));
            }
            EntityType::Monster(ref monster) => {
                try!(writeln!(f, "{}Monster class {}", indent, monster.class));
            }
        }
        // TODO: Presence ...
        try!(writeln!(f, "{}{:?} {:?} {:?}", indent, self.position, self.speed,self.orientation));
        writeln!(f, "{}PV: {}", indent, self.pv)
    }

    ///////////////////////////////////////////////
    //  Accessors
    //

    pub fn get_id(&self) -> Id<Self> {
        self.id
    }

    pub fn get_actor(&self) -> Option<ActorId> {
        self.actor
    }

    pub fn set_actor(&mut self, actor: Option<ActorId>) {
        self.actor = actor;
    }

    pub fn get_position(&self) -> Point2<f32> {
        self.position
    }

    pub fn get_skin(&self) -> u64 {
        self.skin
    }

    pub fn get_pv(&self) -> u64 {
        self.pv
    }

    pub fn get_nominal_speed(&self) -> f32 {
        // For now there is a different (hardcoded) speed for monsters and players
        match self.e_type {
            EntityType::Player(_) => DEFAULT_SPEED,
            EntityType::Monster(_) => DEFAULT_AI_SPEED,
        }
    }

    pub fn get_orientation(&self) -> Direction {
        self.orientation
    }

    pub fn get_type(&self) -> &EntityType {
        &self.e_type
    }

}

// Reason why an action has been rejected
// TODO: Put in lycan-serialize
pub enum Error {
    AlreadyAttacking,
}

#[derive(Debug,Copy,Clone)]
enum AttackState {
    Idle,
    Attacking,
    // A number between 1.0 and 0.0
    // 1.0 means the entity just started reloading
    // When it reaches 0.0 it switches back to the Idle state
    Reloading(f32),
}

#[derive(Debug)]
pub enum EntityType {
    // An entity can be a player
    Player(PlayerData),
    // A monster
    Monster(MonsterData),
}

#[derive(Debug,Clone,Default)]
struct CurrentStats {
    level: u64,
    strength: u64,
    dexterity: u64,
    constitution: u64,
    intelligence: u64,
    precision: u64,
    wisdom: u64,
    speed: f32,
    attack_speed: f32,
}

#[derive(Debug,Clone)]
pub struct PlayerData {
    name: String,
    id: Id<Player>,
    map: Id<Map>,
    experience: u64,
    gold: u64,
    guild: String,
}

#[derive(Debug,Clone)]
pub struct MonsterData {
    class: Id<Monster>,
}

impl PlayerData {
    pub fn get_id(&self) -> Id<Player> {
        self.id
    }
}

impl HasId for Entity {
    type Type = u64;
}

///////////////////////////////////////////////
//  Conversions
//

impl From<Player> for Entity {
    fn from(player: Player) -> Entity {
        let mut entity = Entity::new(
            EntityType::Player(PlayerData {
                name: player.name,
                id: player.id,
                map: player.position.map,
                experience: player.experience,
                gold: player.gold,
                guild: player.guild,
            }),
            Point2::new(player.position.x, player.position.y),
            Direction::East,   // TODO
            player.skin,
            player.stats,
            player.current_pv,
            );
        entity.recompute_current_stats();
        entity
    }
}

impl Into<Option<Player>> for Entity {
    fn into(self) -> Option<Player> {
        let player_data = match self.e_type {
            EntityType::Player(player) => player,
            _ => {
                error!("Attempted to convert a non-player entity to a player");
                return None;
            }
        };
        let position = Position {
            x: self.position.x,
            y: self.position.y,
            map: player_data.map,
        };
        let player = Player {
            id: player_data.id,
            name: player_data.name,
            skin: self.skin,
            current_pv: self.pv,
            position: position,
            experience: player_data.experience,
            gold: player_data.gold,
            guild: player_data.guild,
            stats: self.base_stats,
        };

        Some(player)
    }
}

impl Entity {
    // XXX: Should we really pass instance_id and map?
    pub fn into_management_representation(&self, instance_id: Id<Instance>, map: Id<Map>)
    -> EntityManagement {
        let position = PositionInstance {
            x: self.position.x,
            y: self.position.y,
            map: map,
            instance: instance_id,
        };
        let entity_type = match self.e_type {
            EntityType::Player(ref player) => {
                let player_struct = PlayerStruct {
                    uuid: player.id,
                    name: player.name.clone(),
                    gold: player.gold,
                    guild: player.guild.clone(),
                    experience: player.experience,
                };
                DataEntityType::Player(player_struct)
            }
            EntityType::Monster(ref monster) => {
                let monster_struct = MonsterStruct {
                    monster_class: monster.class,
                    name: "TODO".to_string(),
                    behaviour_tree: "TODO".to_string(),
                };
                DataEntityType::Monster(monster_struct)
            }
        };
        EntityManagement {
            id: self.id,
            entity_type: entity_type,
            skin: self.skin,
            current_pv: self.pv,
            position: position,
            stats: self.base_stats,
        }
    }

    pub fn to_entity_state(&self) -> EntityState {
        EntityState::new(self.id, self.position, self.orientation)
    }

    pub fn to_entity_update(&self) -> EntityUpdate {
        EntityUpdate {
            entity_id: self.id.as_u64(),
            position: self.position,
            speed: self.speed,
            pv: self.pv,
        }
    }
}

///////////////////////////////////////////////
//  Mock entities
//
//  TODO: Remove when it is not needed any more
//

impl Entity {
    pub fn fake_ai(class: Id<Monster>, x: f32, y: f32) -> Entity {
        let stats = Stats {
            level:          1,
            strength:       2,
            dexterity:      3,
            constitution:   4,
            intelligence:   5,
            precision:      6,
            wisdom:         7,
        };
        let skin = ::utils::get_next_skin();
        let monster = MonsterData {
            class: class,
        };
        Entity::new(
            EntityType::Monster(monster),
            Point2::new(x, y),
            Direction::South,
            skin,
            stats,
            100)
    }
}
