use id::Id;
use actor::ActorId;
use std::collections::HashSet;
use entity::{Entity,EntityStore};
use messages::{self,Command,Notification,EntityOrder};


#[derive(Debug)]
pub struct AiActor {
    id: ActorId,
    entities: HashSet<Id<Entity>>,
    // Behaviour Tree
    // Behaviour Tree data
}

impl AiActor {
    pub fn get_id(&self) -> ActorId {
        self.id
    }
    pub fn get_commands(&mut self, _commands: &mut Vec<Command>) {
    }
    pub fn execute_orders(&mut self,
                      entities: &mut EntityStore,
                      notifications: &mut Vec<Notification>,
                      _previous: &[Notification]) {
        // Do something with behaviour trees
    }
    pub fn register_entity(&mut self, entity: Id<Entity>) {
        self.entities.insert(entity);
    }

    pub fn fake() -> AiActor {
        AiActor {
            id: Id::new(),
            entities: Default::default(),
        }
    }
}
