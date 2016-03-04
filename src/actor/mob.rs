use std::collections::HashSet;
use std::fmt::{self,Debug,Formatter};

use behaviour_tree::tree::BehaviourTreeNode;
use id::Id;
use actor::ActorId;
use entity::{Entity,EntityStore};
use messages::{self,Command,Notification,EntityOrder};
use scripts::{BehaviourTree};
use ai::{BehaviourTreeData,Context};

pub struct AiActor {
    id: ActorId,
    entity: Option<Id<Entity>>,
    entities: HashSet<Id<Entity>>, // XXX: Do we really need this?
    tree: BehaviourTree,
    tree_data: BehaviourTreeData,
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
        let me = match self.entity {
            None => {
                warn!("Trying to execute behaviour tree on AI without main entity {}", self.id);
                return;
            }
            Some(me) => me,
        };
        let mut context = Context::new(me, entities, &mut self.tree_data);
        self.tree.visit(&mut context);
    }
    pub fn register_entity(&mut self, entity: Id<Entity>) {
        self.entity = Some(entity);
        self.entities.insert(entity);
    }

    pub fn fake(tree: BehaviourTree) -> AiActor {
        AiActor {
            id: Id::new(),
            entity: None,
            entities: Default::default(),
            tree: tree,
            tree_data: BehaviourTreeData::new(),
        }
    }
}
