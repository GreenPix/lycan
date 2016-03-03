use std::collections::HashSet;
use std::fmt::{self,Debug,Formatter};

use behaviour_tree::tree::BehaviourTreeNode;
use id::Id;
use actor::ActorId;
use entity::{Entity,EntityStore};
use messages::{self,Command,Notification,EntityOrder};
use scripts::{BehaviourTree};
use ai::Context;

pub struct AiActor {
    id: ActorId,
    entities: HashSet<Id<Entity>>,
    tree: BehaviourTree,
    // Behaviour Tree
    // Behaviour Tree data
}

impl Debug for AiActor {
    fn fmt(&self, f: &mut Formatter) -> Result<(),fmt::Error> {
        let tree = "[behaviour tree]";
        f.debug_struct("AiActor")
            .field("id", &self.id)
            .field("entities", &self.entities)
            .field("tree", &tree)
            .finish()
    }
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
        // Context should give access to storage / current game state
        let string = String::from("Hello World");
        let mut context = Context { s: &string };
        self.tree.visit(&mut context);
        // Do something with behaviour trees
    }
    pub fn register_entity(&mut self, entity: Id<Entity>) {
        self.entities.insert(entity);
    }

    pub fn fake(tree: BehaviourTree) -> AiActor {
        AiActor {
            id: Id::new(),
            entities: Default::default(),
            tree: tree,
        }
    }
}
