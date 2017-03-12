use id::{HasForgeableId,HasId,Id};
use uuid::Uuid;

// Intended to be all the info needed to spawn a monster
// TODO
#[derive(Serialize,Deserialize,Debug,Clone)]
pub struct Monster {
    pub monster_class: Id<Monster>,
}

impl HasForgeableId for Monster {}

impl HasId for Monster {
    type Type = Uuid;
}
