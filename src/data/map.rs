use id::{Id, HasForgeableId, HasId};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    pub uuid: Id<Map>,
    pub name: String,
}

impl HasId for Map {
    type Type = Uuid;
}

impl HasForgeableId for Map {}

impl Map {
    pub fn new(id: Id<Map>, name: String) -> Map {
        Map {
            uuid: id,
            name: name,
        }
    }

    pub fn get_id(&self) -> Id<Map> {
        self.uuid
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Used when the "default_fallback" flag is set. It will return a default map, with the given
    /// uuid
    pub fn default_map(uuid: Id<Map>) -> Map {
        Map {
            uuid: uuid,
            name: format!("Default map - {}", uuid),
        }
    }
}

