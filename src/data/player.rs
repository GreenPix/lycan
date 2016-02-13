

use id::{Id,HasForgeableId};
use data::Map;

// Intended to be all the info needed for that player to go in game
#[derive(Debug)]
#[derive(RustcEncodable,RustcDecodable)]
pub struct Player {
    id: Id<Player>,
    map_position: Id<Map>,
    name: String,
}

impl Player {
    pub fn new(id: Id<Player>, map: Id<Map>, name: String) -> Player {
        Player {
            id: id,
            map_position: map,
            name: name,
        }
    }

    pub fn get_id(&self) -> Id<Player> {
        self.id
    }

    pub fn get_map_position(&self) -> Id<Map> {
        self.map_position
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

impl HasForgeableId for Player {}
