use super::Instance;
use data::EntityManagement;

impl Instance {
    pub fn get_entities(&self) -> Vec<EntityManagement> {
        self.entities
            .iter()
            .map(|e| e.into_management_representation(self.id, self.map_id))
            .collect()
    }
}
