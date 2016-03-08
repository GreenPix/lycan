use std::net::{self,SocketAddr};
use std::collections::HashMap;
use std::thread;
use std::io;
use std::boxed::FnBox;

use mio::*;
use mio::tcp::TcpListener;

use utils;
use instance::Instance;
use actor::{NetworkActor,ActorId};
use id::Id;
use data::{Player,Map};
use entity::{Entity,EntityType};
use messages::{Command,Request,NetworkNotification};
use network::Message;
use scripts::{AaribaScripts,BehaviourTrees};

use self::resource_manager::{Error,ResourceManager};
use self::arriving_client::ArrivingClientManager;

mod authentication;
mod resource_manager;
mod arriving_client;

const RESOURCE_MANAGER_THREADS: usize = 2;

const SERVER: Token = Token(0);
const UDP_SOCKET: Token = Token(1);

// XXX: Hack to remove ... currently we consider only one map
lazy_static!{
    static ref UNIQUE_MAP: Map = Map::new(Id::forge(1));
}

#[derive(Debug,Clone)]
pub struct GameParameters {
    pub port: u16,
    pub configuration_url: String,
}

pub struct Game {
    instances: HashMap<Id<Map>, HashMap<Id<Instance>, Sender<Command>>>,
    player_positions: HashMap<Id<Player>, Id<Instance>>,
    server: TcpListener,
    resource_manager: ResourceManager,
    arriving_clients: ArrivingClientManager,
    callbacks: Callbacks,

    // TODO: Should this be integrated with the resource manager?
    scripts: AaribaScripts,
    trees: BehaviourTrees,
}

impl Game {
    fn new(
        server: TcpListener,
        scripts: AaribaScripts,
        trees: BehaviourTrees,
        sender: Sender<Request>,
        base_url: String,
        ) -> Game {
        Game {
            instances: HashMap::new(),
            player_positions: HashMap::new(),
            server: server,
            resource_manager: ResourceManager::new(RESOURCE_MANAGER_THREADS, sender, base_url),
            arriving_clients: ArrivingClientManager::new(),
            callbacks: Callbacks::new(),
            scripts: scripts,
            trees: trees,
        }
    }

    pub fn spawn_game(parameters: GameParameters) -> Result<Sender<Request>,io::Error> {
        let ip = net::IpAddr::V4(Ipv4Addr::new(0,0,0,0));
        let addr = SocketAddr::new(ip,parameters.port);
        let server = try!(TcpListener::bind(&addr));

        // XXX: AN UNWRAP -> to solve when we got time
        let scripts = AaribaScripts::get_from_url(&parameters.configuration_url).unwrap();
        let behaviour_trees = BehaviourTrees::get_from_url(&parameters.configuration_url).unwrap();

        let mut event_loop = try!(EventLoop::new());
        try!(event_loop.register(&server, SERVER, EventSet::all(), PollOpt::level()));
        let sender = event_loop.channel();
        let mut game = Game::new(
            server,
            scripts,
            behaviour_trees,
            sender.clone(),
            parameters.configuration_url.clone(),
            );

        // XXX: Hacks
        let fake_tokens = authentication::generate_fake_authtok();
        for (tok, id) in fake_tokens {
            game.arriving_clients.new_auth_tok(tok, id);
        }
        game.instances.insert(UNIQUE_MAP.get_id(), HashMap::new());

        thread::spawn(move || {
            debug!("Started game");
            event_loop.run(&mut game).unwrap();
            debug!("Stopping game");
        });
        Ok(sender)
    }

    fn apply(&mut self, event_loop: &mut EventLoop<Self>, request: Request) {
        match request {
            Request::Arbitrary(req) => {
                req.execute(self, event_loop);
            }
            Request::UnregisteredActor{actor,entities} => {
                debug!("Unregistered {} {:?}", actor, entities);
                // TODO: Store it or change its map ...

                for entity in entities {
                    let player: Option<Player> = entity.into();
                    if let Some(player) = player {
                        self.player_positions.remove(&player.id);

                        let path = format!("./scripts/entities/{}", player.id);
                        utils::serialize_to_file(path, &player);
                    }

                }
            }
            Request::InstanceShuttingDown(state) => {
                debug!("Instance {} shutting down. State {:?}", state.id, state);
                // TODO: Do something?
            }
            Request::JobFinished(job) => {
                let callbacks = self.callbacks.get_callbacks(job);
                for cb in callbacks {
                    cb.call_box((event_loop, self));
                }
            }
        }
    }

    // Spawn a new instance if needed
    fn assign_actor_to_map(
        &mut self,
        event_loop: &mut EventLoop<Self>,
        map: Id<Map>,
        actor: NetworkActor,
        entities: Vec<Entity>,
        ) {
        match self.instances.get_mut(&map) {
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
                        let (id, instance) = Instance::spawn_instance(
                            event_loop.channel(),
                            self.scripts.clone(),
                            self.trees.clone(),
                            );
                        instance.send(Command::NewClient(actor,entities)).unwrap();
                        Some((id, instance))
                    }
                };

                // Because of the borrow checker
                if let Some((id, instance)) = register_new_instance {
                    instances.insert(id, instance);
                }
            }
            None => {
                error!("Trying to access nonexisting map {}", map);
            }
        }
    }

    fn track_player(&mut self, entities: &[Entity], instance: Id<Instance>) {
        for entity in entities {
            if let EntityType::Player(ref player) = *entity.get_type() {
                self.player_positions.insert(player.get_id(), instance);
            }
        }
    }

    fn player_ready(&mut self, event_loop: &mut EventLoop<Self>,  mut actor: NetworkActor, id: Id<Player>) {
        match self.resource_manager.retrieve_player(id) {
            Ok(entity) => {
                let map = entity.get_map_position().unwrap();
                actor.register_entity(entity.get_id());
                let notification = NetworkNotification::this_is_you(entity.get_id().as_u64());
                actor.queue_message(Message::new(notification));
                self.assign_actor_to_map(event_loop, map, actor, vec![entity]);
            }
            Err(Error::Processing(job)) => {
                self.callbacks.add(job, move |event_loop, game| {
                    game.player_ready(event_loop, actor, id);
                });
            }
            Err(Error::NotFound) => {
                //TODO
                unimplemented!();
            }
        }
    }
}

impl Handler for Game {
    type Message = Request;
    type Timeout = usize;

    fn ready(&mut self, event_loop: &mut EventLoop<Self>, token: Token, event: EventSet) {
        match token {
            SERVER => {
                trace!("Called server with event {:?}", event);
                match self.server.accept() {
                    Err(e) => {
                        error!("Unexpected error when accepting connection {}", e);
                    }
                    Ok(None) => {
                        warn!("Unexpected None received when accepting socket");
                    }
                    Ok(Some((stream, _address))) => {
                        self.arriving_clients.new_client(stream, event_loop);
                    }
                }
            }
            UDP_SOCKET => {
            }
            _token => {
                if let Some((actor, id)) = self.arriving_clients.ready(event_loop, token, event) {
                    self.player_ready(event_loop, actor, id);
                }
            }
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Request) {
        self.apply(event_loop, msg);
    }
}

type Callback = Box<FnBox(&mut EventLoop<Game>, &mut Game) + Send>;

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
    where F: FnOnce(&mut EventLoop<Game>, &mut Game) + 'static + Send {
        self.add_callback_inner(job, Box::new(cb))
    }

    fn add_callback_inner(&mut self, job: usize, cb: Callback) {
        self.callbacks.entry(job).or_insert(Vec::new()).push(cb);
    }

    fn get_callbacks(&mut self, job: usize) -> Vec<Callback> {
        self.callbacks.remove(&job).unwrap_or(Vec::new())
    }
}
