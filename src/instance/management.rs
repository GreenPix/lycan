use super::Instance;
use data::{
    EntityManagement,
    SpawnMonster,
};

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

    pub fn remove_entity(&mut self, entity: u64) -> Result<(),()> {
        Ok(())
        //match self.store.remove
    }
}
