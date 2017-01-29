use std::net::{self,SocketAddr,Ipv4Addr};
use std::collections::HashMap;
use std::thread;
use std::io;
use std::boxed::FnBox;
use std::sync::mpsc::{Receiver,Sender};
use std::rc::{Rc,Weak};
use std::cell::RefCell;
use std::sync::mpsc as std_mpsc;

use tokio_core::reactor::{Core,Handle};
use futures::sync::mpsc::{self,UnboundedReceiver,UnboundedSender};
use futures::future::{self,Future,IntoFuture};
use futures::Stream;

use lycan_serialize::AuthenticationToken;

use utils;
use instance::{InstanceRef,Instance};
use actor::{NetworkActor,ActorId};
use id::{Id,HasId,WeakId};
use data::{Player,Map,EntityManagement,EntityType};
use data::UNIQUE_MAP;
use entity::{Entity};
use messages::{
    Command,
    Request,
    Notification,
    ActorCommand,
};
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
    pub tick_duration: f32,
}

pub struct Game {
    maps: HashMap<Id<Map>, Map>,
    // Keep track of all _active_ (not shuting down) instances, indexed by map ID
    map_instances: HashMap<Id<Map>, HashMap<Id<Instance>, InstanceRef>>,
    // Keep track of all instances still alive
    instances: HashMap<Id<Instance>, InstanceRef>,
    players: HashMap<Id<Player>, EntityManagement>,
    players_ref: HashMap<Id<Player>, Sender<ActorCommand>>,
    resource_manager: ResourceManager,
    authentication_manager: AuthenticationManager,
    sender: UnboundedSender<Request>,
    tick_duration: f32,
    shutdown: bool,
    handle: Handle,
    game_ref: Option<Weak<RefCell<Game>>>,

    // TODO: Should this be integrated with the resource manager?
    scripts: AaribaScripts,
    trees: BehaviourTrees,
}

impl Game {
    fn new(
        scripts: AaribaScripts,
        trees: BehaviourTrees,
        sender: UnboundedSender<Request>,
        base_url: String,
        tick_duration: f32,
        handle: Handle,
        ) -> Game {
        Game {
            maps: HashMap::new(),
            map_instances: HashMap::new(),
            instances: HashMap::new(),
            players: HashMap::new(),
            players_ref: HashMap::new(),
            sender: sender.clone(),
            authentication_manager: AuthenticationManager::new(),
            resource_manager: ResourceManager::new_rest(base_url),
            tick_duration: tick_duration,
            shutdown: false,
            scripts: scripts,
            trees: trees,
            handle: handle,
            game_ref: None,
        }
    }

    pub fn spawn_game(parameters: GameParameters) -> Result<UnboundedSender<Request>,()> {
        let (tx1, rx1) = ::std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let scripts = AaribaScripts::get_from_url(&parameters.configuration_url).unwrap();
            let behaviour_trees = BehaviourTrees::get_from_url(&parameters.configuration_url).unwrap();

            let (sender2, rx2) = mpsc::unbounded();

            let ip = net::IpAddr::V4(Ipv4Addr::new(0,0,0,0));
            let addr = SocketAddr::new(ip,parameters.port);
            network::start_server(addr, sender2.clone());

            management::start_management_api(sender2.clone());
            let mut game = Game::new(
                scripts,
                behaviour_trees,
                sender2.clone(),
                parameters.configuration_url.clone(),
                parameters.tick_duration,
                core.handle(),
                );

            // XXX: Hacks
            //let _ = game.resource_manager.load_map(UNIQUE_MAP.get_id());
            game.map_instances.insert(UNIQUE_MAP.get_id(), HashMap::new());
            game.maps.insert(UNIQUE_MAP.get_id(), UNIQUE_MAP.clone());
            // End hacks
            tx1.send(sender2).unwrap();

            let game = Rc::new(RefCell::new(game));
            {
                let weak_game = Rc::downgrade(&game);
                let mut game_ref = game.borrow_mut();
                (*game_ref).game_ref = Some(weak_game);
            }

            let game_clone = game.clone();
            let fut = rx2.for_each(move |request| {
                // TODO: A way to stop this loop ...
                let mut game_ref = game_clone.borrow_mut();
                game_ref.apply(request);
                Ok(())
            });

            debug!("Started game");
            // This should never return
            core.run(fut).unwrap();

            debug!("Stopping game");
        });

        rx1.recv().map_err(|_| ())
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

                    let (tx, rx) = std_mpsc::channel();
                    let actor = NetworkActor::new(client_id, client, rx);
                    if let Some(old_actor) = self.players_ref.insert(player_id, tx) {
                        // This is the case where a client tries to reconnect with the ID
                        // of a character already in the game
                        //
                        // In this case, we should kick the client associated with that ID,
                        // and replace it with this new client.
                        //
                        // However, it can also happen that the client currently in game is
                        // actually leaving already, or will start to leave shortly (yay 
                        // asynchronicity!) and the two messages (kick/replace, and actor+entity)
                        // will cross each other.
                        //
                        // We thus need to make sure that, whatever the case we are in, the
                        // newly connected client will eventually get connected to the right
                        // entity, and that no two entities have the same player ID at the
                        // same time

                        // TODO: Unimplemented ...
                        error!("Unimplemented multiple connections for same player ID {}", player_id);
                        // For now kick both clients
                        // This will however break Sarosa in Authentication-less mode (the client
                        // will not know why it has been kicked)
                        drop(actor);
                        let _ = old_actor.send(ActorCommand::Kick);
                        // Note: the Sender<ActorCommand> will be invalid (we just dropped the
                        // newly-created associated actor) but this should not cause any problems
                        // It should get removed when the connected entity comes back
                    } else {
                        self.player_ready(actor, player_id);
                    }
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
                            self.tick_duration,
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
        use self::resource_manager::ResultExt;

        let game_ref = self.game_ref.clone().unwrap();
        let fut = self.resource_manager.get_player(id)
            .and_then(move |entity| {
                let map = entity.get_map_position().unwrap();
                actor.register_entity(entity.get_id());
                let notification = Notification::this_is_you(entity.get_id().as_u64());
                actor.send_message(notification);
                let game_ref = game_ref.upgrade().ok_or_else(|| "Upgrading Weak to Rc failed")?;
                let mut game = game_ref.borrow_mut();
                game.assign_actor_to_map(map, actor, vec![entity]);
                Ok(())
            }).map_err(|e| {
                error!("Error while processing player: {}", e);
            });

        self.handle.spawn(fut);
    }

    fn entity_leaving(&mut self, entity: Entity) {
        let player: Option<Player> = entity.into();
        if let Some(player) = player {
            self.players.remove(&player.id);

            // Make sure this handles double-connexion problems
            self.players_ref.remove(&player.id);

            let path = format!("./scripts/entities/{}", player.id);
            utils::serialize_to_file(path, &player);
        }
    }

    fn connect_character(&mut self, id: Id<Player>, token: AuthenticationToken) {
        self.authentication_manager.add_token(id, token);
        //self.resource_manager.load_player(id);
    }

    pub fn verify_token(&mut self, id: Id<Player>, token: AuthenticationToken) -> bool {
        self.authentication_manager.verify_token(id, token)
    }

    fn get_active_maps(&self) -> Vec<Map> {
        self.maps.values().cloned().collect()
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
