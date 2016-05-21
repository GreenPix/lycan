use super::Instance;
use data::{
    EntityManagement,
    SpawnMonster,
};
use id::WeakId;
use entity::Entity;

impl Instance {
    pub fn get_entities(&self) -> Vec<EntityManagement> {
        self.entities
            .iter()
            .map(|e| e.into_management_representation(self.id, self.map_id))
            .collect()
    }

    pub fn spawn_monster(&mut self, monster: SpawnMonster) -> EntityManagement {
        let id = self.add_fake_ai(monster.x, monster.y);
        self.entities.get(id).unwrap().into_management_representation(self.id, self.map_id)
    }

    pub fn remove_entity(&mut self, entity: WeakId<Entity>) -> Result<(),RemoveEntityError> {
        let mut found = false;
        match self.entities.remove_if(entity, |e| { found = true; e.is_monster() }) {
            None => Err(if found { RemoveEntityError::IsPlayer } else { RemoveEntityError::NotFound }),
            Some(_e) => {
                // Send back to game?

                // TODO: Notification of entity leaving
                // TODO: Kick corresponding actor
                Ok(())
            }
        }
    }
}

pub enum RemoveEntityError {
    NotFound,
    IsPlayer,
}

