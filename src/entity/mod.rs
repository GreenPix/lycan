use std::fmt::{self,Formatter};
use std::cell::{RefCell,RefMut,Ref};
use std::sync::atomic::{AtomicUsize, Ordering};

use nalgebra::{Pnt2,Vec2};
use rand;

use id::{Id,HasForgeableId,HasId};
use data::{
    Map,Player,Stats,Position,
    EntityManagement, PositionInstance,
    PlayerStruct, MonsterStruct,
    EntityType as DataEntityType,
};
use messages::{EntityState};
use instance::Instance;

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

#[derive(Debug)]
pub struct Entity {
    id: Id<Entity>,

    e_type: EntityType,
    position: Pnt2<f32>,
    // We probably won't save the speed ...
    speed: Vec2<f32>,
    orientation: Direction,
    skin: u64,
    pv: u64,
    hitbox: RectangleHitbox,
    attack_box: RectangleHitbox,
    attack_offset_x: Vec2<f32>,
    attack_offset_y: Vec2<f32>,
    base_stats: Stats,
    stats: CurrentStats,

    // TODO: Replace by a FSM
    walking: bool,
    attacking: u64, // XXX: This is currently expressed in tick, not ms!
}

lazy_static! {
    static ref NEXT_SKIN: AtomicUsize = AtomicUsize::new(0);
}

impl Entity {
    pub fn new(e_type: EntityType,
               position: Pnt2<f32>,
               orientation: Direction,
               skin: u64,
               base_stats: Stats,
               pv: u64,
               )
        -> Entity {
            let mut e = Entity {
                id: Id::new(),

                e_type: e_type,
                position: position,
                speed: Vec2::new(0.0,0.0),
                orientation: orientation,
                base_stats: base_stats,
                stats: Default::default(),
                skin: skin,
                pv: pv,
                hitbox: RectangleHitbox::new_default(),
                attack_box: RectangleHitbox::new(0.5, 0.5),
                attack_offset_x: Vec2::new(0.75, 0.0),
                attack_offset_y: Vec2::new(0.0, 1.0),

                walking: false,
                attacking: 0,
            };
            e.recompute_current_stats();
            e
        }

    pub fn get_id(&self) -> Id<Self> {
        self.id
    }

    pub fn get_position(&self) -> Pnt2<f32> {
        self.position
    }

    pub fn get_skin(&self) -> u64 {
        self.skin
    }

    pub fn get_pv(&self) -> u64 {
        self.pv
    }

    pub fn get_orientation(&self) -> Direction {
        self.orientation
    }

    pub fn get_type(&self) -> &EntityType {
        &self.e_type
    }

    pub fn is_player(&self) -> bool {
        if let EntityType::Player(_) = self.e_type {
            true
        } else {
            false
        }
    }

    pub fn is_monster(&self) -> bool {
        if let EntityType::Invoked(_) = self.e_type {
            true
        } else {
            false
        }
    }

    // Takes effects into account
    pub fn recompute_current_stats(&mut self) {
        let speed = match self.e_type {
            EntityType::Player(_) => DEFAULT_SPEED,
            EntityType::Invoked(_) => DEFAULT_AI_SPEED,
        };
        self.stats.speed = speed;
        self.stats.strength = self.base_stats.strength;
        self.stats.dexterity = self.base_stats.dexterity;
        self.stats.constitution = self.base_stats.constitution;
        self.stats.intelligence = self.base_stats.intelligence;
        self.stats.precision = self.base_stats.precision;
        self.stats.wisdom = self.base_stats.wisdom;
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

    // TODO: Remove
    pub fn fake_player(id: Id<Player>) -> Entity {
        let stats = Stats {
            level:          1,
            strength:       2,
            dexterity:      3,
            constitution:   4,
            intelligence:   5,
            precision:      6,
            wisdom:         7,
        };
        let position = Position {
            x: 0.0,
            y: 0.0,
            map: Id::forge(1)
        };
        let name = format!("Player {}", id);
        let skin = NEXT_SKIN.fetch_add(1, Ordering::Relaxed) as u64;
        let player = Player {
            id:         id,
            name:       name,
            skin:       skin,
            current_pv: 100,
            position:   position,
            experience: 0,
            gold:       0,
            guild:      String::new(),
            stats:      stats,
        };
        Entity::from(player)
    }

    pub fn fake_ai(x: f32, y: f32) -> Entity {
        let stats = Stats {
            level:          1,
            strength:       2,
            dexterity:      3,
            constitution:   4,
            intelligence:   5,
            precision:      6,
            wisdom:         7,
        };
        let skin = NEXT_SKIN.fetch_add(1, Ordering::Relaxed) as u64;
        Entity::new(
            EntityType::Invoked(None),
            Pnt2::new(x, y),
            Direction::South,
            skin,
            stats,
            100)
    }

    pub fn to_entity_state(&self) -> EntityState {
        EntityState::new(self.id, self.position, self.orientation)
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
            EntityType::Invoked(ref parent) => {
                match *parent {
                    Some(parent) => {
                        try!(writeln!(f, "{}Invoked entity attached to {}", indent, parent));
                    }
                    None => {
                        try!(writeln!(f, "{}Invoked entity", indent));
                    }
                }
            }
        }
        // TODO: Presence ...
        try!(writeln!(f, "{}{:?} {:?} {:?}", indent, self.position, self.speed,self.orientation));
        writeln!(f, "{}PV: {}", indent, self.pv)
    }

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
                    id: player.id,
                    name: player.name.clone(),
                    gold: player.gold,
                    guild: player.guild.clone(),
                    experience: player.experience,
                };
                DataEntityType::Player(player_struct)
            }
            EntityType::Invoked(_) => {
                let monster_struct = MonsterStruct {
                    monster_class: Id::forge(Default::default()),
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
}

#[derive(Debug)]
pub enum EntityType {
    // An entity can be a player
    Player(PlayerData),
    // Or invoked, with an optional parent
    // XXX: Is the parent really useful?
    Invoked(Option<u64>),
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

impl PlayerData {
    pub fn get_id(&self) -> Id<Player> {
        self.id
    }
}

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
            Pnt2::new(player.position.x, player.position.y),
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
            _ => return None,
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

impl HasId for Entity {
    type Type = u64;
}
