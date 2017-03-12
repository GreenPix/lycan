use uuid::Uuid;

use id::{Id,HasForgeableId,HasId};
use data::{
    Map,
    DEFAULT_MAP_ID,
};

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

    /// Used when the "default_fallback" flag is set. It will return a default player, with the given
    /// uuid
    pub fn default_player(uuid: Id<Player>) -> Player {
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
            map: *DEFAULT_MAP_ID
        };
        let name = format!("Player {}", uuid);
        let skin = ::utils::get_next_skin();
        Player {
            id:         uuid,
            name:       name,
            skin:       skin,
            current_pv: 100,
            position:   position,
            experience: 0,
            gold:       0,
            guild:      String::new(),
            stats:      stats,
        }
    }
}

impl HasForgeableId for Player {}

impl HasId for Player {
    type Type = Uuid;
}
