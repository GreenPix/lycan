use std::collections::hash_map::{self,HashMap,Entry};
use std::thread;
use std::fmt::{Formatter,Display,Error};
use std::io;
use std::mem;
use std::time::Duration as StdDuration;

use mio::*;
use time::{Duration,SteadyTime};

use id::Id;
use entity::{self,Entity,EntityStore};
use actor::{NetworkActor,ActorId,AiActor};
use messages::{self,Command,Notification,Request};
use network::Message;
use scripts::{BehaviourTrees,AaribaScripts};
use data::Map;

mod management;

lazy_static! {
    // 16.666 ms (60 Hz)
    pub static ref SEC_PER_UPDATE: f32 = {
        DEFAULT_REFRESH_PERIOD.num_microseconds().unwrap() as f32 / 1000000.0
    };
    static ref DEFAULT_REFRESH_PERIOD: Duration = Duration::microseconds(16666);
    //static ref DEFAULT_REFRESH_PERIOD: Duration = Duration::milliseconds(1000);
}

#[derive(Debug,Default)]
struct Actors {
    // Design questions: separate all actors? Put them in enum? Use trait objects?
    internal_actors: HashMap<ActorId, AiActor>,
    external_actors: HashMap<ActorId, NetworkActor>,
}

impl Actors {
    fn register_client(&mut self, actor: NetworkActor) {
        let id = actor.get_id();
        match self.external_actors.entry(id) {
            Entry::Occupied(mut entry) => {
                error!("Erasing old actor {:?}", entry.get());
                entry.insert(actor);
            }
            Entry::Vacant(entry) => {
                entry.insert(actor);
            }
        }
    }

    fn register_internal(&mut self, actor: AiActor) {
        let id = actor.get_id();
        match self.internal_actors.entry(id) {
            Entry::Occupied(mut entry) => {
                error!("Erasing old actor {:?}", entry.get());
                entry.insert(actor);
            }
            Entry::Vacant(entry) => {
                entry.insert(actor);
            }
        }

    }

    fn unregister_client(&mut self, id: ActorId) -> Option<NetworkActor> {
        self.external_actors.remove(&id)
    }

    fn broadcast_notifications<H:Handler>(&mut self,
                                          event_loop: &mut EventLoop<H>,
                                          notifications: &[Notification]) {
        let messages: Vec<_> = notifications.iter().map(|notif| {
            Message::new(notif.clone().into())
        }).collect();
        // TODO: "Diffusion lists" to precise what an actor should see
        for (_, actor) in self.external_actors.iter_mut() {
            for message in messages.iter() {
                actor.send_message(event_loop, message.clone());
            }
        }
    }

    fn get_commands(&mut self) -> Vec<Command> {
        // XXX Is this a good idea to do it this way?
        let mut commands_buffer = Vec::new();
        for (_, actor) in self.external_actors.iter_mut() {
            actor.get_commands(&mut commands_buffer);
        }

        for (_, actor) in self.internal_actors.iter_mut() {
            actor.get_commands(&mut commands_buffer);
        }
        commands_buffer
    }

    fn execute_orders(&mut self,
                      entities: &mut EntityStore,
                      notifications: &mut Vec<Notification>,
                      previous: &[Notification]) {
        for (_, actor) in self.external_actors.iter_mut() {
            actor.execute_orders(entities, notifications, previous);
        }
        for (_, actor) in self.internal_actors.iter_mut() {
            actor.execute_orders(entities, notifications, previous);
        }
    }

    fn assign_entity_to_actor(&mut self, actor: ActorId, entity: Id<Entity>) -> bool {
        if let Some(actor) = self.external_actors.get_mut(&actor) {
            actor.register_entity(entity);
            return true;
        }
        if let Some(actor) = self.internal_actors.get_mut(&actor) {
            actor.register_entity(entity);
            return true;
        }
        false
    }

    fn ready<H:Handler>(&mut self, event_loop: &mut EventLoop<H>, token: Token, event: EventSet) {
        let id_u64 = token.as_usize() as u64;
        trace!("Called ready {} with event {:?}", id_u64, event);
        let client = self.external_actors.get_mut(&id_u64)
            .expect("Called ready but no corresponding client in the hashmap");
        client.ready(event_loop, event);
    }

    // TODO: rewrite correctly
    fn dump(&self, f: &mut Formatter, entities: &EntityStore) -> Result<(),Error> {
        let mut indent;
        for actor in self.external_actors.values() {
            indent = "    ";
            try!(actor.dump(f, indent));
            for entity_id in actor.entities_iter() {
                indent = "        ";
                try!(match entities.get(*entity_id) {
                    None => write!(f, "{}ERROR: Inconsistency found!", indent),
                    Some(entity) => {
                        entity.dump(f, indent)
                    }
                });
            }
        }
        Ok(())
    }

    fn drain_external(&mut self) -> hash_map::Drain<ActorId,NetworkActor> {
        self.external_actors.drain()
    }
}


pub struct Instance {
    id: Id<Instance>,

    map_id: Id<Map>,
    entities: EntityStore,
    actors: Actors,
    request: Sender<Request>,
    last_tick: SteadyTime,
    lag: Duration,
    // We will need the previous notifications for AI
    prev_notifications: Vec<Notification>,
    next_notifications: Vec<Notification>,
    scripts: AaribaScripts,
    trees: BehaviourTrees,
    shutting_down: bool,

    // XXX: Do we need to change the refresh period?
    refresh_period: Duration,
}

impl Instance {
    pub fn spawn_instance(request: Sender<Request>,
                          scripts: AaribaScripts,
                          trees: BehaviourTrees,
                          map_id: Id<Map>,
                          ) -> (Id<Self>, Sender<Command>) {
        let mut instance = Instance::new(request, scripts, trees, map_id);
        let mut config = EventLoopConfig::default();
        config.timer_tick_ms((instance.refresh_period.num_milliseconds() / 2) as u64);
        let mut event_loop = EventLoop::configured(config).unwrap();
        let id = instance.get_id();
        let sender = event_loop.channel();
        thread::spawn(move || {
            debug!("Started instance {}", instance.id);
            event_loop.timeout_ms(InstanceTick::CalculateTick,
                                  instance.refresh_period.num_milliseconds() as u64)
                      .unwrap();
            instance.last_tick = SteadyTime::now();
            event_loop.run(&mut instance).unwrap();
            debug!("Stopping instance {}", instance.id);
        });
        (id, sender)
    }

    fn new(request: Sender<Request>,
           scripts: AaribaScripts,
           trees: BehaviourTrees,
           map_id: Id<Map>,
           ) -> Instance {
        let mut instance = Instance {
            id: Id::new(),
            map_id: map_id,
            entities: EntityStore::new(),
            actors: Default::default(),
            request: request,
            last_tick: SteadyTime::now(),
            lag: Duration::zero(),
            refresh_period: *DEFAULT_REFRESH_PERIOD,
            prev_notifications: Default::default(),
            next_notifications: Default::default(),
            scripts: scripts,
            trees: trees,
            shutting_down: false,
        };

        // XXX Fake an AI on the map
        instance.add_fake_ai();
        instance
    }

    fn apply(&mut self, event_loop: &mut EventLoop<Self>, command: Command) {
        match command {
            Command::NewClient(actor,entities) => {
                self.register_client(event_loop, actor, entities);
            }
            Command::Shutdown => {
                self.shutdown(Some(event_loop));
            }
            Command::UnregisterActor(id) => {
                self.unregister_client(event_loop, id);
            }
            Command::Arbitrary(command) => {
                command.execute(self, event_loop);
            }
            Command::AssignEntity((actor,entity)) => {
                self.assign_entity_to_actor(actor, entity);
            }
        }
    }

    fn register_client(
        &mut self,
        event_loop: &mut EventLoop<Self>,
        mut actor: NetworkActor,
        entities: Vec<Entity>,
        ) {
        let id = actor.get_id();
        trace!("Registering actor {} in instance {}", id, self.id);
        match actor.register(event_loop) {
            Ok(_) => {
                for entity in self.entities.iter() {
                    let position = entity.get_position();
                    let skin = entity.get_skin();
                    let entity_id = entity.get_id().as_u64();
                    let pv = entity.get_pv();
                    let notification = Notification::new_entity(entity_id, position, skin, pv);
                    let message = Message::new(notification.into());
                    actor.send_message(event_loop, message);
                }
                for entity in entities {
                    let entity_id = entity.get_id().as_u64();
                    let position = entity.get_position();
                    let skin = entity.get_skin();
                    let pv = entity.get_pv();
                    let notification = Notification::new_entity(entity_id, position, skin, pv);
                    self.next_notifications.push(notification);
                    self.entities.push(entity);
                }
                self.actors.register_client(actor);
            }
            Err(e) => {
                error!("Failed to register actor {}: {}", id, e);
            }
        }
    }

    fn unregister_client(&mut self, event_loop: &mut EventLoop<Self>, id: ActorId) {
        match self.actors.unregister_client(id) {
            Some(actor) => {
                if let Err(e) = actor.deregister(event_loop) {
                    // Can be normal if the connection was lost
                    if !actor.is_connected() && e.kind() == io::ErrorKind::NotFound {
                        trace!("Failed to unregister dropped connection {} (normal operation)", id);
                    } else {
                        error!("Error when enregistering actor: {}", e);
                    }
                }
                // TODO: Check first if the actor needs to be sent back to the Game
                let mut entities = Vec::new();
                for entity_id in actor.entities_iter() {
                    match self.entities.remove(*entity_id) {
                        Some(entity) => {
                            entities.push(entity);
                            let notification = Notification::entity_has_quit(entity_id.as_u64());
                            self.next_notifications.push(notification);
                        },
                        None => error!("Instance {}: Inconsistency between actor {} and its entities: \
                                       entity {} is not present in the map array",
                                       self.id, actor.get_id(), entity_id),
                    }
                }
                self.request.send(Request::UnregisteredActor{actor: actor,entities: entities})
                    .map_err(|e| format!("Failed to send unregistered actor: {:?}", e)).unwrap();
            }
            None => error!("Instance {}: trying to unregister absent actor {}",
                           self.id, id),
        }
    }

    fn shutdown(&mut self, mut event_loop: Option<&mut EventLoop<Self>>) {
        let mut state = ShuttingDownState::new(self.id);
        for (actor_id, actor) in self.actors.drain_external() {
            // Weird thing ... cannot write Some(ref mut ev) ...
            match event_loop.take() {
                Some(ev) => {
                    let _ = actor.deregister(ev);
                    event_loop = Some(ev);
                },
                None => {}
            }
            let mut entities = Vec::new();
            for entity_id in actor.entities_iter() {
                match self.entities.remove(*entity_id) {
                    Some(e) => entities.push(e),
                    None => {
                        error!("Instance {}: Inconsistency between actor {} and its entities: \
                                    entity {} is not present in the map array",
                                    self.id, actor_id, entity_id);
                    }
                }
            }

            state.push(actor, entities);
        }

        let mut request_opt = Some(Request::InstanceShuttingDown(state));
        while let Some(request) = request_opt.take() {
            if let Err(e) = self.request.send(request) {
                match e {
                    NotifyError::Io(e) => {
                        error!("IO Error when sending back state: {}", e);
                    }
                    NotifyError::Full(req) => {
                        ::std::thread::sleep(StdDuration::from_millis(100));
                        // Retry
                        request_opt = Some(req);
                    }
                    NotifyError::Closed(Some(state2)) => {
                        // TODO: Something to do with the state we got back?
                        error!("The Game instance has hung up!");
                    }
                    NotifyError::Closed(None) => {
                        error!("The Game instance has hung up!");
                    }
                }
            }
        }
        self.shutting_down = true;
        event_loop.map(|e| e.shutdown());

        // TODO: The event loop will not exit immediately ... we should handle that
    }

    fn assign_entity_to_actor(&mut self, id: ActorId, entity: Entity) {
        let entity_id = entity.get_id();
        let position = entity.get_position();
        let skin = entity.get_skin();
        let pv = entity.get_pv();
        if self.actors.assign_entity_to_actor(id, entity_id) {
            self.entities.push(entity);
            let notification = Notification::new_entity(entity_id.as_u64(), position, skin, pv);
            self.next_notifications.push(notification);
        } else {
            // Could be normal operation if the actor has just been unregistered (race
            // condition)
            warn!("Missing actor {} when sending entity {}", id, entity.get_id());
            // TODO: Should send back to the Game
            unimplemented!();
        }
        debug!("{}", self);
    }

    pub fn get_id(&self) -> Id<Self> {
        self.id
    }

    fn calculate_tick(&mut self, event_loop: &mut EventLoop<Self>) {
        trace!("Instance {}: Calculating tick\n{}", self.id, self);
        self.actors.execute_orders(&mut self.entities,
                                   &mut self.next_notifications,
                                   &self.prev_notifications);

        entity::update(&mut self.entities, &mut self.next_notifications, &self.scripts);

        let commands_buffer = self.actors.get_commands();
        for command in commands_buffer {
            self.apply(event_loop, command);
        }
        self.actors.broadcast_notifications(event_loop, &self.next_notifications);
        debug!("Notifications: {:?}", self.next_notifications);
        self.prev_notifications.clear();
        mem::swap(&mut self.prev_notifications, &mut self.next_notifications);
    }

    fn add_fake_ai(&mut self) {
        let ai = AiActor::fake(self.trees.generate_tree("zombie").unwrap());
        let id = ai.get_id();
        self.actors.register_internal(ai);

        let entity = Entity::fake_ai();
        self.assign_entity_to_actor(id, entity);
    }
}

impl Handler for Instance {
    type Timeout = InstanceTick;
    type Message = Command;

    fn ready(&mut self, event_loop: &mut EventLoop<Self>, token: Token, event: EventSet) {
        self.actors.ready(event_loop, token, event);
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Command) {
        debug!("Received command {:?}", msg);
        self.apply(event_loop, msg);
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<Self>, action: InstanceTick) {
        match action {
            InstanceTick::CalculateTick => {
                let current = SteadyTime::now();
                let elapsed = current - self.last_tick;
                self.lag = self.lag + elapsed;
                while self.lag >= self.refresh_period {
                    self.calculate_tick(event_loop);
                    self.lag = self.lag - self.refresh_period;
                }
                // TODO: Should we check if we should do a few more iterations?
                self.last_tick = current;
                let sleep = (self.refresh_period - self.lag).num_milliseconds() as u64;
                event_loop.timeout_ms(InstanceTick::CalculateTick, sleep).unwrap();
            }
        }
    }

    fn interrupted(&mut self, _event_loop: &mut EventLoop<Self>) {
        error!("Interrupted");
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let indent = "";
        try!(write!(f, "{}Instance {}:\n", indent, self.id));
        self.actors.dump(f, &self.entities)
    }
}

/// Regular or delayed operations that will execute on an Instance
pub enum InstanceTick {
    /// The main operation will be calculating the next tick
    ///
    /// This will among other things execute all actions made by players
    /// since the last tick, resolve AI trees and send the update to players
    CalculateTick,
}

#[derive(Debug)]
pub struct ShuttingDownState {
    pub id: Id<Instance>,
    pub was_saved: bool,
    pub external_actors: Vec<(NetworkActor,Vec<Entity>)>,
    //pub internal_actors: Vec<(Actor,Vec<Entity>)>,
}

impl ShuttingDownState {
    pub fn new(id: Id<Instance>) -> ShuttingDownState {
        ShuttingDownState {
            id: id,
            was_saved: false,
            external_actors: Vec::new(),
        }
    }

    pub fn push(&mut self, actor: NetworkActor, entities: Vec<Entity>) {
        self.external_actors.push((actor, entities));
    }
}

impl Drop for ShuttingDownState {
    fn drop(&mut self) {
        if !self.was_saved {
            // The state has not been processed and saved
            // This is our last chance to save all the modifications somewhere
            error!("Failed to save the state of instance {}\n{:#?}", self.id, self.external_actors);
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.shutting_down {
            error!("Instance {} has not been shutdown properly", self.id);
            self.shutdown(None);
        }
    }
}
