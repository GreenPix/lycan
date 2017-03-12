use time::Tm;

use data::player::{Stats};
use id::{Id,HasForgeableId};
use data::{Map,Player,Monster};
use entity::Entity;
use instance::Instance;

// Representation of an entity in the management API
#[derive(Serialize,Debug,Clone)]
pub struct EntityManagement {
    pub id: Id<Entity>,
    #[serde(rename="type")]
    pub entity_type: EntityType,
    pub skin: u64,
    pub current_pv: u64,
    pub position: PositionInstance,
    pub stats: Stats,
}

#[derive(Serialize,Debug,Clone)]
pub enum EntityType {
    #[serde(rename="player")]
    Player(PlayerStruct),
    #[serde(rename="monster")]
    Monster(MonsterStruct),
}

#[derive(Serialize,Debug,Clone)]
pub struct PlayerStruct {
    pub uuid: Id<Player>,
    pub name: String,
    pub gold: u64,
    pub experience: u64,
    pub guild: String,
}

#[derive(Serialize,Debug,Clone)]
pub struct MonsterStruct {
    pub monster_class: Id<Monster>,
    pub name: String,
    pub behaviour_tree: String,
}

// Same as data::Position but with instance information
#[derive(Serialize,Debug,Clone,Copy)]
pub struct PositionInstance {
    pub map: Id<Map>,
    pub x: f32,
    pub y: f32,
    pub instance: Id<Instance>,
}

#[derive(Deserialize,Debug,Clone,Copy)]
pub struct SpawnMonster {
    pub monster_class: Id<Monster>,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Deserialize,Debug,Clone)]
pub struct ConnectCharacterParam {
    pub token: String,
    pub id: Id<Player>,
}

#[derive(Serialize,Debug,Clone)]
pub struct GetInstances {
    pub id: Id<Instance>,
    pub map: Id<Map>,
    pub created_at: String,
}

#[derive(Serialize,Debug,Clone)]
pub struct GetMaps {
    pub uuid: Id<Map>,
    pub name: String,
}
