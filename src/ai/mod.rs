use std::collections::HashMap;

use nalgebra::{Vec2,FloatPnt};

use behaviour_tree::tree::{BehaviourTreeNode};
use behaviour_tree::tree::{LeafNodeFactory,VisitResult};
use behaviour_tree::parser::Value;
use behaviour_tree::FactoryProducer;

use id::Id;
use entity::{Entity,EntityStore,Direction};

pub type ActionNode = Box<for<'a, 'b> BehaviourTreeNode<Context<'a, 'b>> + Send>;
//pub type ActionNodeFactory = Box<LeafNodeFactory<Output=Box<for<'a> BehaviourTreeNode<Context<'a>>>>>;
pub type ActionNodeFactory = Box<BoxedClone<Output=ActionNode>>;
pub type ActionNodeFactoryFactory = fn(&Option<Value>) -> Result<ActionNodeFactory,String>;

pub trait BoxedClone: LeafNodeFactory + Send {
    fn boxed_clone(&self) -> ActionNodeFactory;
}

impl Clone for ActionNodeFactory {
    fn clone(&self) -> ActionNodeFactory {
        (**self).boxed_clone()
    }
}
impl <T> BoxedClone for T
where T: Clone,
      T: 'static,
      T: Send,
      T: LeafNodeFactory<Output=ActionNode> {
    fn boxed_clone(&self) -> ActionNodeFactory {
        Box::new(self.clone())
    }
}

pub struct Context<'a, 'b> {
    pub me: Id<Entity>,
    pub entities: &'a mut EntityStore,
    pub storage: &'b mut BehaviourTreeData,
}

impl <'a, 'b> Context<'a, 'b> {
    pub fn new(
        me: Id<Entity>,
        entities: &'a mut EntityStore,
        storage: &'b mut BehaviourTreeData,
        ) -> Context<'a, 'b> {
        Context {
            me: me,
            entities: entities,
            storage: storage,
        }
    }
}

#[derive(Debug,Clone)]
pub struct BehaviourTreeData {
    map: HashMap<String,StoreKind>,
    target: Option<Id<Entity>>,
    path: Option<Path>,
}

impl BehaviourTreeData {
    pub fn new() -> BehaviourTreeData {
        BehaviourTreeData {
            map: HashMap::new(),
            target: None,
            path: None,
        }
    }

    fn set_target(&mut self, target: Option<Id<Entity>>) {
        self.target = target;
    }
}

#[derive(Clone,Debug)]
pub enum StoreKind {
    // TODO
}

// TODO
#[derive(Clone,Debug)]
pub struct Path;

#[derive(Clone)]
pub struct Prototype<T> {
    pub inner: T,
}

impl <T> Prototype<T> {
    pub fn new(inner: T) -> Prototype<T> {
        Prototype {
            inner: inner,
        }
    }
}

impl <T> LeafNodeFactory for Prototype<T>
where T: Clone, 
      T: 'static,
      T: Send,
      T: for <'a,'b> BehaviourTreeNode<Context<'a,'b>> {
    type Output = ActionNode;
    fn instanciate(&self) -> Self::Output {
        Box::new(self.inner.clone())
    }
}


#[derive(Debug,Clone)]
pub struct PrintText {
    pub text: String,
}

impl <'a,'b> BehaviourTreeNode<Context<'a,'b>> for PrintText {
    fn visit(&mut self, _context: &mut Context) -> VisitResult {
        println!("Message node: {}", self.text);
        VisitResult::Success
    }
}

pub fn print_text(options: &Option<Value>) -> Result<ActionNodeFactory, String> {
    let message_orig = match options {
        &Some(Value::String(ref message)) => message,
        other => return Err(format!("Expected message, found {:?}", other)),
    };

    let message = message_orig.replace("_"," ");

    Ok(Box::new(Prototype::new(PrintText { text: message })))
}

#[derive(Clone)]
pub struct GetClosestTarget {
    max_sqdistance: f32,
}

impl <'a,'b> BehaviourTreeNode<Context<'a,'b>> for GetClosestTarget {
    fn visit(&mut self, context: &mut Context) -> VisitResult {
        let (me, others) = match context.entities.get_mut_wrapper(context.me) {
            None => {
                warn!("Main entity {} was not found in entities list", context.me);
                return VisitResult::Failure;
            }
            Some((me, others)) => (me, others),
        };
        let my_position = me.get_position();
        let mut closest_other = None;
        let mut closest_other_sqdistance = self.max_sqdistance;
        for other in others.iter() {
            let sqdistance = my_position.sqdist(&other.get_position());
            if sqdistance < closest_other_sqdistance {
                closest_other = Some(other.get_id());
                closest_other_sqdistance = sqdistance;
            }
        }
        context.storage.target = closest_other;
        debug!("Get closest target: found {:?} at sqdist {}", closest_other, closest_other_sqdistance);
        VisitResult::Success
    }
}

pub fn get_closest_target(options: &Option<Value>) -> Result<ActionNodeFactory, String> {
    // TODO
    Ok(Box::new(Prototype::new(GetClosestTarget { max_sqdistance: 10000.0 })))
}

#[derive(Clone)]
// TODO: Timeout? Max distance? Stop condition?
pub struct WalkToTarget;

impl <'a,'b> BehaviourTreeNode<Context<'a,'b>> for WalkToTarget {
    fn visit(&mut self, context: &mut Context) -> VisitResult {
        // TODO: Proper pathfinding

        let (me, mut others) = match context.entities.get_mut_wrapper(context.me) {
            None => {
                warn!("Main entity {} was not found in entities list", context.me);
                return VisitResult::Failure;
            }
            Some((me, others)) => (me, others),
        };
        let target = match context.storage.target {
            None => return VisitResult::Failure,
            Some(id) => match others.get(id) {
                None => {
                    warn!("Could not find target {}", id);
                    me.walk(None);
                    return VisitResult::Failure;
                }
                Some(o) => o,
            }
        };
        let vector = target.get_position() - me.get_position();
        let abs_diff_x = vector.x.abs();
        let abs_diff_y = vector.y.abs();
        match me.get_orientation() {
            Direction::East | Direction::West => {
                if abs_diff_x > abs_diff_y/2.0 {
                    if vector.x.is_sign_positive() {
                        me.walk(Some(Direction::East));
                    } else {
                        me.walk(Some(Direction::West));
                    }
                } else {
                    if vector.y.is_sign_positive() {
                        me.walk(Some(Direction::North));
                    } else {
                        me.walk(Some(Direction::South));
                    }
                }
            }
            Direction::North | Direction::South => {
                if abs_diff_x/2.0 > abs_diff_y {
                    if vector.x.is_sign_positive() {
                        me.walk(Some(Direction::East));
                    } else {
                        me.walk(Some(Direction::West));
                    }
                } else {
                    if vector.y.is_sign_positive() {
                        me.walk(Some(Direction::North));
                    } else {
                        me.walk(Some(Direction::South));
                    }
                }
            }
        }
        VisitResult::Running
    }
}

pub fn walk_to_target(options: &Option<Value>) -> Result<ActionNodeFactory, String> {
    // TODO
    Ok(Box::new(Prototype::new(WalkToTarget)))
}


#[derive(Default)]
pub struct LeavesCollection {
    inner: HashMap<String,ActionNodeFactoryFactory>,
}

macro_rules! insert_all {
    ($($name:expr => $fun:expr),*) => (
        {
            let mut collection = LeavesCollection::new();
            $(
            collection.inner.insert(
                String::from($name),
                $fun,
                );
            )*
            collection
        }
        );
    ($($name:expr => $fun:expr),+,) => (
        insert_all!($($name => $fun),+)
        );
}

impl LeavesCollection {
    pub fn new() -> LeavesCollection {
        LeavesCollection {
            inner: HashMap::new(),
        }
    }

    pub fn register_function(
        &mut self,
        key: String,
        f: ActionNodeFactoryFactory,
        ) {
        self.inner.insert(key,f);
    }

    pub fn standard() -> LeavesCollection {
        let collection = insert_all!(
            "print_text" => print_text,
            "get_closest_target" => get_closest_target,
            "walk_to_target" => walk_to_target,
            //"increment" => increment,

            );

        collection
    }
}

impl FactoryProducer for LeavesCollection {
    type Factory = ActionNodeFactory;
    fn generate_leaf(&self, name: &str, option: &Option<Value>) -> Result<Self::Factory,String> {
        match self.inner.get(name) {
            None => Err(format!("Could not find leaf with name {}", name)),
            Some(fact_fact) => {
                let fact = try!(fact_fact(option));
                Ok(fact) 
            }
        }
    }
}

