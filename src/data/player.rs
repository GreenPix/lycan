

use id::{Id,HasForgeableId};
use data::Map;

// Intended to be all the info needed for that player to go in game
#[derive(Serialize,Deserialize,Debug,Clone)]
pub struct Player {
    pub id: Id<Player>,
    pub name: String,
    //class
    pub skin: u64,
    pub current_pv: u64,
    pub position: Position,
    pub experience: u64,
    pub gold: u64,
    //group
    pub guild: String,
    pub stats: Stats,
}

#[derive(Serialize,Deserialize,Debug,Clone,Copy)]
pub struct Stats {
    pub level: u64,
    pub strength: u64,
    pub dexterity: u64,
    pub constitution: u64,
    pub intelligence: u64,
    pub precision: u64,
    pub wisdom: u64,
}

#[derive(Serialize,Deserialize,Debug,Clone,Copy)]
pub struct Position {
    pub map: Id<Map>,
    pub x: f32,
    pub y: f32,
}

impl Player {
    pub fn get_id(&self) -> Id<Player> {
        self.id
    }

    pub fn get_map_position(&self) -> Id<Map> {
        self.position.map
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

impl HasForgeableId for Player {}

