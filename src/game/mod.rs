use std::net::{self,SocketAddr,Ipv4Addr};
use std::collections::hash_map::{HashMap,Entry};
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
use error_chain::ChainedError;

use lycan_serialize::AuthenticationToken;

use utils;
use instance::{InstanceRef,Instance};
use actor::{NetworkActor,ActorId};
use id::{Id,HasId,WeakId};
use data::{Player,Map,EntityManagement,EntityType};
use entity::{Entity};
use messages::{
    Command,
    Request,
    Notification,
    ActorCommand,
};
use network::{self,Client};
use scripts::{AaribaScripts,BehaviourTrees};

use self::resource_manager::{Error,ResourceManager};
use self::authentication::AuthenticationManager;

mod authentication;
mod resource_manager;
mod management;

#[derive(Debug,Clone)]
pub struct GameParameters {
    pub port: u16,
    pub configuration_url: String,
    pub tick_duration: f32,
    pub default_fallback: bool,
}

pub struct Game {
    //maps: HashMap<Id<Map>, Map>,
    // Keep track of all _active_ (not shuting down) instances, indexed by map ID
    maps: ActiveMaps,
    //map_instances: HashMap<Id<Map>, HashMap<Id<Instance>, InstanceRef>>,
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
    game_ref: Weak<RefCell<Game>>,

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
        default_fallback: bool,
        ) -> Game {
        Game {
            maps: ActiveMaps::new(),
            instances: HashMap::new(),
            players: HashMap::new(),
            players_ref: HashMap::new(),
            sender: sender.clone(),
            authentication_manager: AuthenticationManager::new(),
            resource_manager: ResourceManager::new_rest(base_url, default_fallback),
            tick_duration: tick_duration,
            shutdown: false,
            scripts: scripts,
            trees: trees,
            handle: handle,
            game_ref: Weak::new(),
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
            let game = Game::new(
                scripts,
                behaviour_trees,
                sender2.clone(),
                parameters.configuration_url.clone(),
                parameters.tick_duration,
                core.handle(),
                parameters.default_fallback,
                );

            tx1.send(sender2).unwrap();

            let game = Rc::new(RefCell::new(game));
            {
                let weak_game = Rc::downgrade(&game);
                let mut game_ref = game.borrow_mut();
                (*game_ref).game_ref = weak_game;
            }

            let game_clone = game.clone();
            let fut = rx2.for_each(move |request| {
                // TODO: A way to stop this loop ...
                let mut game_ref = game_clone.borrow_mut();
                game_ref.apply(request);
                Ok(())
            });

            debug!("Started game");
            // This currently never returns
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
                self.new_client(client);
            }
        }
        // Default: don't exit
        false
    }

    fn start_shutdown(&mut self) {
        self.shutdown = true;
        self.maps.broadcast_shutdown();

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
        if !self.maps.has_map(map) {
            let map_id = map;
            let game_ref = self.game_ref.clone();
            let fut = self.resource_manager.get_map(map)
                .then(move |map_res| {
                    let game_ref = game_ref.upgrade()
                        .expect("Upgrading Weak to Rc failed");
                    let mut game = game_ref.borrow_mut();
                    match map_res {
                        Ok(map) => {
                            let _ = game.maps.add_map(map);
                            game.assign_actor_to_map(
                                map_id,
                                actor,
                                entities);
                            Ok(())
                        }
                        Err(e) => {
                            error!("Error while loading map {}, dropping new actor {}",
                                   e.display(), actor.get_id());
                            game.actor_leaving(actor, entities);
                            Err(())
                        }
                    }
                });
            self.handle.spawn(fut);
        } else {
            let instance = self.maps.get_map_instance(map,
                                                      &self.sender,
                                                      &self.scripts,
                                                      &self.trees,
                                                      self.tick_duration)
                .expect("ActiveMap.has_map() returned true, but then failed to get an instance??");
            instance.send(Command::NewClient(actor, entities)).unwrap();
        }
    }

    fn new_client(&mut self, client: Client) {
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

    fn player_ready(&mut self, mut actor: NetworkActor, id: Id<Player>) {
        let game_ref = self.game_ref.clone();
        let fut = self.resource_manager.get_player(id)
            .then(move |entity_res| {
                let game_ref = game_ref.upgrade()
                    .expect("Upgrading Weak to Rc failed");
                let mut game = game_ref.borrow_mut();
                match entity_res {
                    Ok(entity) => {
                        let map = entity.get_map_position().unwrap();
                        actor.register_entity(entity.get_id());
                        let notification = Notification::this_is_you(entity.get_id().as_u64());
                        actor.send_message(notification);
                        game.assign_actor_to_map(map, actor, vec![entity]);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Error while processing player: {}", e.display());
                        game.players.remove(&id);
                        if let Some(sender) = game.players_ref.remove(&id) {
                            let _ = sender.send(ActorCommand::Kick);
                        }
                        Err(())
                    }
                }
            });

        self.handle.spawn(fut);
    }

    fn actor_leaving(&mut self, _actor: NetworkActor, entities: Vec<Entity>) {
        for entity in entities {
            self.entity_leaving(entity);
        }
    }

    fn entity_leaving(&mut self, entity: Entity) {
        let player: Option<Player> = entity.into();
        if let Some(player) = player {
            debug!("Player leaving: {}", player.id);
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
        self.maps.get_active_maps()
    }
}

struct ActiveMaps {
    inner: HashMap<Id<Map>, (Map, HashMap<Id<Instance>, InstanceRef>)>,
}

impl ActiveMaps {
    fn new() -> ActiveMaps {
        ActiveMaps {
            inner: HashMap::new(),
        }
    }

    // Add a map to the pool of available maps
    fn add_map(&mut self, map: Map) -> Result<(),()> {
        match self.inner.entry(map.get_id()) {
            Entry::Vacant(e) => {
                e.insert((map, HashMap::new()));
                Ok(())
            }
            Entry::Occupied(_) => {
                error!("Tried to re-add a map that was already loaded: {}", map.get_id());
                Err(())
            }
        }
    }

    fn has_map(&self, map: Id<Map>) -> bool {
        self.inner.contains_key(&map)
    }

    /// Get existing instance, or if necessary spawn a new one
    /// If the map data was not available, return an error
    fn get_map_instance(&mut self,
                        map_id: Id<Map>,
                        sender: &UnboundedSender<Request>,
                        scripts: &AaribaScripts,
                        trees: &BehaviourTrees,
                        tick_duration: f32,
                        ) -> Result<&InstanceRef, ()> {
        match self.inner.get_mut(&map_id) {
            None => {
                error!("Tried to spawn an instance on a non-available map {}", map_id);
                Err(())
            }
            Some(&mut (ref map, ref mut hashmap)) => {
                // FIXME: Non-Lexical Lifetimes would help here
                if !hashmap.is_empty() {
                    // Unwrap here should never fail
                    let (_id, instance) = hashmap.iter().nth(0).unwrap();
                    return Ok(instance);
                }
                let instance = Instance::spawn_instance(
                    sender.clone(),
                    scripts.clone(),
                    trees.clone(),
                    map_id,
                    tick_duration,
                    );

                let instance_id = instance.get_id();
                let instance_ref = hashmap.entry(instance_id)
                    .or_insert(instance);
                Ok(instance_ref)
            }
        }
    }

    fn broadcast_shutdown(&mut self) {
        for (_id, (_map, instances)) in self.inner.drain() {
            for instance in instances.values() {
                let _ = instance.send(Command::Shutdown);
            }
        }
    }

    fn get_active_maps(&self) -> Vec<Map> {
        self.inner.values().map(|&(ref map, _)| map.clone()).collect()
    }
}

impl HasId for Game {
    type Type = u64;
}
