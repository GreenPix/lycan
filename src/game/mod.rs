use std::net::{self,SocketAddr};
use std::collections::HashMap;
use std::thread;
use std::io;

use mio::*;
use mio::tcp::TcpListener;

use instance::Instance;
use actor::{NetworkActor,ActorId};
use id::Id;
use data::Map;
use entity::Entity;
use messages::{Command,Request,NetworkNotification};
use network::Message;
use scripts::AaribaScripts;

use self::resource_manager::ResourceManager;
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
    actors_positions: HashMap<ActorId, Id<Instance>>,
    server: TcpListener,
    resource_manager: ResourceManager,
    arriving_clients: ArrivingClientManager,

    // TODO: Should this be integrated with the resource manager?
    scripts: AaribaScripts,
}

impl Game {
    fn new(server: TcpListener, scripts: AaribaScripts) -> Game {
        Game {
            instances: HashMap::new(),
            actors_positions: HashMap::new(),
            server: server,
            resource_manager: ResourceManager::new(RESOURCE_MANAGER_THREADS),
            arriving_clients: ArrivingClientManager::new(),
            scripts: scripts,
        }
    }

    pub fn spawn_game(parameters: GameParameters) -> Result<Sender<Request>,io::Error> {
        // Those items are now deprecated in libstd, but mio still uses them
        let ip = net::IpAddr::V4(Ipv4Addr::new(0,0,0,0));
        let addr = SocketAddr::new(ip,parameters.port);
        let server = try!(TcpListener::bind(&addr));

        // XXX: AN UNWRAP -> to solve when we got time
        let scripts = AaribaScripts::get_from_url(&parameters.configuration_url).unwrap();

        let mut event_loop = try!(EventLoop::new());
        try!(event_loop.register(&server, SERVER, EventSet::all(), PollOpt::level()));
        let sender = event_loop.channel();
        let mut game = Game::new(server, scripts);

        // XXX: Hacks
        let fake_tokens = authentication::generate_fake_authtok();
        for (tok, id) in fake_tokens {
            game.resource_manager.load_player(id);
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
            }
            Request::InstanceShuttingDown(state) => {
                debug!("Instance {} shutting down. State {:?}", state.id, state);
                // TODO: Do something?
            }
        }
    }

    // Spawn a new instance if needed
    fn assign_actor_to_map(&mut self, event_loop: &mut EventLoop<Self>, map: Id<Map>, actor: NetworkActor) {
        match self.instances.get_mut(&map) {
            Some(instances) => {
                // TODO: Load balancing
                let register_new_instance = match instances.iter_mut().nth(0) {
                    Some((_id, instance)) => {
                        // An instance is already there, send the actor to it
                        instance.send(Command::NewClient(actor)).unwrap();
                        None
                    }
                    None => {
                        // No instance for this map, spawn one
                        let (id, instance) = Instance::spawn_instance(
                            event_loop.channel(),
                            self.scripts.clone());
                        instance.send(Command::NewClient(actor)).unwrap();
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
                if let Some((mut actor, id)) = self.arriving_clients.ready(event_loop, token, event) {
                    match self.resource_manager.retrieve_player(id) {
                        Err(_e) => {
                            //TODO
                            unimplemented!();
                        }
                        Ok(entity) => {
                            let map = entity.get_map_position().unwrap();
                            actor.register_entity(entity.get_id());
                            let notification = NetworkNotification::this_is_you(entity.get_id().as_u64());
                            actor.queue_message(Message::new(notification));
                            actor.push_entity(entity);
                            self.assign_actor_to_map(event_loop, map, actor);
                        }
                    }
                }
            }
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Request) {
        self.apply(event_loop, msg);
    }
}
