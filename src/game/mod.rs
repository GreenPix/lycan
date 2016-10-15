use std::net::{self,SocketAddr,Ipv4Addr};
use std::collections::HashMap;
use std::thread;
use std::io;
use std::boxed::FnBox;
use std::sync::mpsc::{self,Receiver,Sender};

use lycan_serialize::AuthenticationToken;

use utils;
use instance::{InstanceRef,Instance};
use actor::{NetworkActor,ActorId};
use id::{Id,HasId,WeakId};
use data::{Player,Map,EntityManagement,EntityType};
use data::UNIQUE_MAP;
use entity::{Entity};
use messages::{Command,Request,Notification};
use network;
use scripts::{AaribaScripts,BehaviourTrees};

use self::resource_manager::{Error,ResourceManager};
use self::authentication::AuthenticationManager;

mod authentication;
mod resource_manager;
//mod arriving_client;
mod management;

const RESOURCE_MANAGER_THREADS: usize = 2;

#[derive(Debug,Clone)]
pub struct GameParameters {
    pub port: u16,
    pub configuration_url: String,
}

pub struct Game {
    // Keep track of all _active_ (not shuting down) instances, indexed by map ID
    map_instances: HashMap<Id<Map>, HashMap<Id<Instance>, InstanceRef>>,
    // Keep track of all instances still alive
    instances: HashMap<Id<Instance>, InstanceRef>,
    players: HashMap<Id<Player>, EntityManagement>,
    resource_manager: ResourceManager,
    authentication_manager: AuthenticationManager,
    sender: Sender<Request>,
    callbacks: Callbacks,
    shutdown: bool,

    // TODO: Should this be integrated with the resource manager?
    scripts: AaribaScripts,
    trees: BehaviourTrees,
}

impl Game {
    fn new(
        scripts: AaribaScripts,
        trees: BehaviourTrees,
        sender: Sender<Request>,
        base_url: String,
        ) -> Game {
        Game {
            map_instances: HashMap::new(),
            instances: HashMap::new(),
            players: HashMap::new(),
            sender: sender.clone(),
            authentication_manager: AuthenticationManager::new(),
            resource_manager: ResourceManager::new(RESOURCE_MANAGER_THREADS, sender, base_url),
            callbacks: Callbacks::new(),
            shutdown: false,
            scripts: scripts,
            trees: trees,
        }
    }

    pub fn spawn_game(parameters: GameParameters) -> Result<Sender<Request>,io::Error> {
        let scripts = AaribaScripts::get_from_url(&parameters.configuration_url).unwrap();
        let behaviour_trees = BehaviourTrees::get_from_url(&parameters.configuration_url).unwrap();

        let (sender, rx) = mpsc::channel();

        let ip = net::IpAddr::V4(Ipv4Addr::new(0,0,0,0));
        let addr = SocketAddr::new(ip,parameters.port);
        network::start_server(addr, sender.clone());

        management::start_management_api(sender.clone());
        let mut game = Game::new(
            scripts,
            behaviour_trees,
            sender.clone(),
            parameters.configuration_url.clone(),
            );

        // XXX: Hacks
        game.authentication_manager.fake_authentication_tokens();
        let _ = game.resource_manager.load_map(UNIQUE_MAP.get_id());
        game.map_instances.insert(UNIQUE_MAP.get_id(), HashMap::new());
        // End hacks

        thread::spawn(move || {
            // This is the "event loop"
            debug!("Started game");
            for request in rx {
                if game.apply(request) {
                    break;
                }
            }
            debug!("Stopping game");
        });
        Ok(sender)
    }

    // Returns true to exit the loop
    fn apply(&mut self, request: Request) -> bool {
        match request {
            Request::Arbitrary(req) => {
                req.execute(self);
            }
            Request::UnregisteredActor{actor,entities} => {
                debug!("Unregistered {} {:?}", actor, entities);
                // TODO: Store it or change its map ...

                for entity in entities {
                    self.entity_leaving(entity);
                }
            }
            Request::InstanceShuttingDown(mut state) => {
                debug!("Instance {} shutting down. State {:?}", state.id, state);
                self.instances.remove(&state.id);
                for (_actor, entities) in state.external_actors.drain(..) {
                    for entity in entities {
                        self.entity_leaving(entity);
                    }
                    // Drop the client without goodbye?
                }
                state.was_saved = true;

                if self.shutdown && self.instances.is_empty() {
                    return true;
                }
            }
            Request::JobFinished(job) => {
                let callbacks = self.callbacks.get_callbacks(job);
                for cb in callbacks {
                    cb.call_box((self,));
                }
            }
            Request::PlayerUpdate(players) => {
                for player in players {
                    let id = if let EntityType::Player(ref p) = player.entity_type {
                        p.uuid
                    } else {
                        continue;
                    };
                    self.players.insert(id, player);
                }
            }
            Request::NewClient(client) => {
                if self.shutdown {
                    // Drop the client
                } else {
                    let player_id = Id::forge(client.uuid);
                    // TODO: Generate earlier? (during the connexion, in the network code)
                    let client_id = Id::new();
                    let actor = NetworkActor::new(client_id, client);
                    self.player_ready(actor, player_id);
                }
            }
        }
        // Default: don't exit
        false
    }

    fn start_shutdown(&mut self) {
        self.shutdown = true;
        for (_id, instances) in self.map_instances.drain() {
            for instance in instances.values() {
                let _ = instance.send(Command::Shutdown);
            }
        }

        // TODO: Shutdown the network side
        // At the moment, there is no clean way of doing this
    }

    // Spawn a new instance if needed
    fn assign_actor_to_map(
        &mut self,
        map: Id<Map>,
        actor: NetworkActor,
        entities: Vec<Entity>,
        ) {
        match self.map_instances.get_mut(&map) {
            Some(instances) => {
                // TODO: Load balancing
                let register_new_instance = match instances.iter_mut().nth(0) {
                    Some((_id, instance)) => {
                        // An instance is already there, send the actor to it
                        instance.send(Command::NewClient(actor,entities)).unwrap();
                        None
                    }
                    None => {
                        // No instance for this map, spawn one
                        let instance = Instance::spawn_instance(
                            self.sender.clone(),
                            self.scripts.clone(),
                            self.trees.clone(),
                            map,
                            );
                        instance.send(Command::NewClient(actor,entities)).unwrap();
                        Some(instance)
                    }
                };

                // Because of the borrow checker
                if let Some(instance) = register_new_instance {
                    let id = instance.get_id();
                    instances.insert(id, instance.clone());
                    self.instances.insert(id, instance);
                }
            }
            None => {
                error!("Trying to access nonexisting map {}", map);
            }
        }
    }

    fn player_ready(&mut self, mut actor: NetworkActor, id: Id<Player>) {
        match self.resource_manager.retrieve_player(id) {
            Ok(entity) => {
                let map = entity.get_map_position().unwrap();
                actor.register_entity(entity.get_id());
                let notification = Notification::this_is_you(entity.get_id().as_u64());
                actor.send_message(notification);
                self.assign_actor_to_map(map, actor, vec![entity]);
            }
            Err(Error::Processing(job)) => {
                self.callbacks.add(job, move |game| {
                    game.player_ready(actor, id);
                });
            }
            Err(Error::NotFound) => {
                //TODO
                unimplemented!();
            }
        }
    }

    fn entity_leaving(&mut self, entity: Entity) {
        let player: Option<Player> = entity.into();
        if let Some(player) = player {
            self.players.remove(&player.id);

            let path = format!("./scripts/entities/{}", player.id);
            utils::serialize_to_file(path, &player);
        }
    }

    fn connect_character(&mut self, id: Id<Player>, token: AuthenticationToken) {
        self.authentication_manager.add_token(id, token);
        self.resource_manager.load_player(id);
    }

    pub fn verify_token(&mut self, id: Id<Player>, token: AuthenticationToken) -> bool {
        self.authentication_manager.verify_token(id, token)
    }
}

type Callback = Box<FnBox(&mut Game) + Send>;

struct Callbacks {
    callbacks: HashMap<usize, Vec<Callback>>,
}

impl Callbacks {
    fn new() -> Callbacks {
        Callbacks {
            callbacks: HashMap::new(),
        }
    }

    fn add<F>(&mut self, job: usize, cb: F)
    where F: FnOnce(&mut Game) + 'static + Send {
        self.add_callback_inner(job, Box::new(cb))
    }

    fn add_callback_inner(&mut self, job: usize, cb: Callback) {
        self.callbacks.entry(job).or_insert(Vec::new()).push(cb);
    }

    fn get_callbacks(&mut self, job: usize) -> Vec<Callback> {
        self.callbacks.remove(&job).unwrap_or(Vec::new())
    }
}

impl HasId for Game {
    type Type = u64;
}
