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
}

