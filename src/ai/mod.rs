use std::collections::HashMap;

use behaviour_tree::tree::{BehaviourTreeNode};
use behaviour_tree::tree::{LeafNodeFactory,VisitResult};
use behaviour_tree::parser::Value;
use behaviour_tree::FactoryProducer;

pub type ActionNode = Box<for<'a> BehaviourTreeNode<Context<'a>>>;
//pub type ActionNodeFactory = Box<LeafNodeFactory<Output=Box<for<'a> BehaviourTreeNode<Context<'a>>>>>;
pub type ActionNodeFactory = Box<BoxedClone<Output=Box<for<'a> BehaviourTreeNode<Context<'a>>>>>;
pub type ActionNodeFactoryFactory = fn(&Option<Value>) -> Result<ActionNodeFactory,String>;

pub trait BoxedClone: LeafNodeFactory {
    fn boxed_clone(&self) -> ActionNodeFactory;
}

impl Clone for ActionNodeFactory {
    fn clone(&self) -> ActionNodeFactory {
        (**self).boxed_clone()
    }
}
impl <T: ?Sized> BoxedClone for T
where T: Clone,
      T: 'static,
      T: LeafNodeFactory<Output=Box<for<'a> BehaviourTreeNode<Context<'a>>>> {
    fn boxed_clone(&self) -> ActionNodeFactory {
        Box::new(self.clone())
    }
}

pub struct Context<'a> {
    pub s: &'a str,
}

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
      T: for <'a> BehaviourTreeNode<Context<'a>> {
    type Output = ActionNode;
    fn instanciate(&self) -> Self::Output {
        Box::new(self.inner.clone())
    }
}


#[derive(Debug,Clone)]
pub struct PrintText {
    pub text: String,
}

impl <'a> BehaviourTreeNode<Context<'a>> for PrintText {
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

