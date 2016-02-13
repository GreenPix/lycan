use id::{Id, HasForgeableId};

#[derive(Clone, Copy, Debug)]
pub struct Map {
    id: Id<Map>,
}

impl HasForgeableId for Map {}

impl Map {
    pub fn new(id: Id<Map>) -> Map {
        Map {
            id: id,
        }
    }

    pub fn get_id(&self) -> Id<Map> {
        self.id
    }
}

