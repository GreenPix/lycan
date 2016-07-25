// Aariba scripts and behaviour trees used by Lycan
use std::collections::HashMap;

use aariba::rules::RulesEvaluator;
use aariba;
use hyper::error::Error as HyperError;

use behaviour_tree::tree::factory::TreeFactory;
use behaviour_tree::tree::{LeafNodeFactory,BehaviourTreeNode};
use behaviour_tree;

use ai::{Context,ActionNode,ActionNodeFactory,LeavesCollection};
use utils;

pub type BehaviourTreeFactory = TreeFactory<ActionNodeFactory>;
pub type BehaviourTree = behaviour_tree::BehaviourTree<ActionNode>;

#[derive(Debug,Clone)]
pub struct AaribaScripts {
    pub combat: RulesEvaluator,
}

#[derive(Debug)]
pub enum Error {
    Hyper(HyperError),
    AaribaParsing(String),
    BehaviourTreeParsing(String),
}

impl From<HyperError> for Error {
    fn from(e: HyperError) -> Error {
        Error::Hyper(e)
    }
}

impl AaribaScripts {
    pub fn get_from_url(base_url: &str) -> Result<AaribaScripts,Error> {
        let mut url = String::from(base_url);
        url.push_str("/combat.aariba");
        debug!("Getting file {}", url);
        let script = try!(utils::get_file_from_url(&url));
        let parsed_script =
            try!(aariba::parse_rule(&script)
                .map_err(Error::AaribaParsing));
        let scripts = AaribaScripts {
            combat: parsed_script,
        };
        Ok(scripts)
    }
}

#[derive(Clone)]
pub struct BehaviourTrees {
    inner: HashMap<String, BehaviourTreeFactory>,
}

impl BehaviourTrees {
    // TODO: An append command
    pub fn get_from_url(base_url: &str) -> Result<BehaviourTrees,Error> {
        let mut url = String::from(base_url);
        url.push_str("/zombie.bt");
        debug!("Getting file {}", url);
        let script = try!(utils::get_file_from_url(&url));
        let mut map = HashMap::new();
        let leaves = LeavesCollection::standard();
        let parsed_trees =
            try!(behaviour_tree::parse(&script,&leaves)
                .map_err(Error::BehaviourTreeParsing));
        for tree in parsed_trees {
            let name = String::from(tree.get_name());
            map.insert(name,tree);
        }
        let trees = BehaviourTrees {
            inner: map,
        };

        Ok(trees)
    }

    pub fn generate_factory(&self, name: &str) -> Option<BehaviourTreeFactory> {
        self.inner.get(name).map(|f| f.clone())
    }

    pub fn generate_tree(&self, name: &str) -> Option<BehaviourTree> {
        self.inner.get(name).map(|f| f.optimize())
    }
}
