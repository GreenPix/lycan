use std::collections::hash_map::{self,HashMap,Entry};
use std::thread;
use std::fmt::{Formatter,Display,Error};
use std::io;
use std::mem;
use std::time::Duration as StdDuration;
use std::sync::mpsc::{self,Receiver,Sender};

use time::{self,Duration,SteadyTime,Tm};
use schedule_recv;

use id::{Id,HasId};
use entity::{self,Entity,EntityStore};
use actor::{NetworkActor,ActorId,AiActor};
use messages::{self,Command,Notification,Request};
use scripts::{BehaviourTrees,AaribaScripts};
use data::{Map,Monster};

pub mod management;

lazy_static! {
    static ref GAME_PLAYER_REFRESH_PERIOD: Duration = Duration::seconds(2);
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

    fn unregister_ai(&mut self, id: ActorId) -> Option<AiActor> {
        self.internal_actors.remove(&id)
    }

    fn broadcast_notifications(&mut self,
                               notifications: &[Notification]) {
        for client in self.external_actors.values_mut() {
            for notif in notifications {
                client.send_message(notif.clone());
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
    created_at: Tm,

    tick_duration: f32,
    tick_id: u64,
}

impl Instance {
    pub fn spawn_instance(request: Sender<Request>,
                          scripts: AaribaScripts,
                          trees: BehaviourTrees,
                          map_id: Id<Map>,
                          tick_duration: f32,
                          ) -> InstanceRef {
        let mut instance = Instance::new(request, scripts, trees, map_id, tick_duration);
        let id = instance.get_id();
        let created_at = instance.created_at;
        let (sender, rx) = mpsc::channel();
        thread::spawn(move || {
            let tick = schedule_recv::periodic(StdDuration::from_millis((tick_duration * 1000.0) as u64));
            let players_update = schedule_recv::periodic(GAME_PLAYER_REFRESH_PERIOD.to_std().unwrap());
            instance.last_tick = SteadyTime::now();

            debug!("Started instance {}", instance.id);
            loop {
                select! {
                    _ = tick.recv() => {
                        trace!("Received tick notification");
                        let refresh_period = Duration::microseconds((instance.tick_duration * 1_000_000.0) as i64);
                        let current = SteadyTime::now();
                        let elapsed = current - instance.last_tick;
                        instance.lag = instance.lag + elapsed;
                        let mut loop_count = 0;
                        while instance.lag >= refresh_period {
                            instance.calculate_tick();
                            instance.lag = instance.lag - refresh_period;
                            loop_count += 1;
                        }
                        if loop_count != 1 {
                            debug!("Needed to adjust the tick rate! loop count {}", loop_count);
                        }
                        // TODO: Should we check if we should do a few more iterations?
                        instance.last_tick = current;
                    },
                    _ = players_update.recv() => {
                        let vec = instance.entities
                            .iter()
                            .filter(|e| e.is_player())
                            .map(|e| e.into_management_representation(instance.id, instance.map_id))
                            .collect();
                        instance.request.send(Request::PlayerUpdate(vec)).unwrap();
                    },
                    command = rx.recv() => {
                        let command = command.unwrap();
                        println!("Received command {:?}", command);
                        if instance.apply(command) {
                            break;
                        }
                    }
                }
            }
            debug!("Stopping instance {}", instance.id);
        });
        InstanceRef::new(id, sender, created_at, map_id)
    }

    fn new(request: Sender<Request>,
           scripts: AaribaScripts,
           trees: BehaviourTrees,
           map_id: Id<Map>,
           tick_duration: f32,
           ) -> Instance {
        use uuid::Uuid;

        let mut instance = Instance {
            id: Id::new(),
            map_id: map_id,
            entities: EntityStore::new(),
            actors: Default::default(),
            request: request,
            last_tick: SteadyTime::now(),
            lag: Duration::zero(),
            tick_duration: tick_duration,
            prev_notifications: Default::default(),
            next_notifications: Default::default(),
            scripts: scripts,
            trees: trees,
            shutting_down: false,
            created_at: time::now_utc(),
            tick_id: 0,
        };

        // XXX Fake an AI on the map
        let class_str = "67e6001e-d735-461d-b32e-2e545e12b3d2";
        let uuid = Uuid::parse_str(class_str).unwrap();
        instance.add_fake_ai(Id::forge(uuid), 0.0, 0.0);
        instance
    }

    // Apply a command to update the game state.
    //
    // returns: true if the instance has been shutdown while executing
    // the command, false otherwise
    fn apply(&mut self, command: Command) -> bool {
        match command {
            Command::NewClient(actor,entities) => {
                self.register_client(actor, entities);
            }
            Command::Shutdown => {
                self.shutdown();
            }
            Command::UnregisterActor(id) => {
                self.unregister_client(id);
            }
            Command::Arbitrary(command) => {
                command.execute(self);
            }
            Command::AssignEntity((actor,entity)) => {
                self.assign_entity_to_actor(actor, entity);
            }
        }

        self.shutting_down
    }

    fn register_client(
        &mut self,
        mut actor: NetworkActor,
        entities: Vec<Entity>,
        ) {
        let id = actor.get_id();
        trace!("Registering actor {} in instance {}", id, self.id);
        for entity in self.entities.iter() {
            let position = entity.get_position();
            let skin = entity.get_skin();
            let entity_id = entity.get_id().as_u64();
            let pv = entity.get_pv();
            let ns = entity.get_nominal_speed();
            let notification = Notification::new_entity(entity_id, position, skin, pv, ns);
            actor.send_message(notification);
        }
        for entity in entities {
            let entity_id = entity.get_id().as_u64();
            let position = entity.get_position();
            let skin = entity.get_skin();
            let pv = entity.get_pv();
            let ns = entity.get_nominal_speed();
            let notification = Notification::new_entity(entity_id, position, skin, pv, ns);
            self.next_notifications.push(notification);
            self.entities.push(entity);
        }
        self.actors.register_client(actor);
    }

    fn unregister_client(&mut self, id: ActorId) {
        match self.actors.unregister_client(id) {
            Some(actor) => {
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

    fn shutdown(&mut self) {
        let mut state = ShuttingDownState::new(self.id);
        for (actor_id, actor) in self.actors.drain_external() {
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

        if let Err(e) = self.request.send(Request::InstanceShuttingDown(state)) {
            // TODO: Something to do with the state we got back?
            error!("The Game instance has hung up!\n{:#?}", e);
        }
        self.shutting_down = true;
    }

    fn assign_entity_to_actor(&mut self, id: ActorId, mut entity: Entity) {
        let entity_id = entity.get_id();
        let position = entity.get_position();
        let skin = entity.get_skin();
        let pv = entity.get_pv();
        let ns = entity.get_nominal_speed();
        if self.actors.assign_entity_to_actor(id, entity_id) {
            entity.set_actor(Some(id));
            self.entities.push(entity);
            let notification = Notification::new_entity(entity_id.as_u64(), position, skin, pv, ns);
            self.next_notifications.push(notification);
        } else {
            // Could be normal operation if the actor has just been unregistered (race
            // condition)
            warn!("Missing actor {} when sending entity {}", id, entity.get_id());
            // TODO: Should send back to the Game
        }
        debug!("{}", self);
    }

    pub fn get_id(&self) -> Id<Self> {
        self.id
    }

    fn calculate_tick(&mut self) {
        trace!("Instance {}: Calculating tick\n{}", self.id, self);
        self.actors.execute_orders(&mut self.entities,
                                   &mut self.next_notifications,
                                   &self.prev_notifications);

        let events = entity::update(
            &mut self.entities,
            &mut self.next_notifications,
            &self.scripts,
            self.tick_id,
            self.tick_duration,
            );
        for event in events {
            self.process_event(event);
        }

        let commands_buffer = self.actors.get_commands();
        for command in commands_buffer {
            self.apply(command);
        }
        self.actors.broadcast_notifications(&self.next_notifications);
        debug!("Notifications: {:?}", self.next_notifications);
        self.prev_notifications.clear();
        mem::swap(&mut self.prev_notifications, &mut self.next_notifications);
        self.tick_id += 1;
    }

    fn process_event(&mut self, event: TickEvent) {
        match event {
            TickEvent::EntityDeath(dead_entity) => {
                self.next_notifications.push(Notification::Death {
                    entity: dead_entity.get_id().as_u64(),
                });

                // Trick to make sarosa make the entity disappear
                // Should not be needed when the client is modified to handle death properly
                // (animation etc)
                self.next_notifications.push(Notification::EntityHasQuit {
                    entity: dead_entity.get_id().as_u64(),
                });
                // TODO: Send entity back to actor
            }
        }
    }

    fn add_fake_ai(&mut self, class: Id<Monster>, x: f32, y: f32) -> Id<Entity> {
        let ai = AiActor::fake(self.trees.generate_tree("zombie").unwrap());
        let id = ai.get_id();
        self.actors.register_internal(ai);

        let mut entity = Entity::fake_ai(class, x, y);
        entity.set_actor(Some(id));
        let entity_id = entity.get_id();
        self.assign_entity_to_actor(id, entity);
        entity_id
    }
}

#[derive(Clone)]
pub struct InstanceRef {
    id: Id<Instance>,
    sender: Sender<Command>,
    created_at: Tm,
    map: Id<Map>,
}

impl InstanceRef {
    pub fn new(id: Id<Instance>,
               sender: Sender<Command>,
               created_at: Tm,
               map: Id<Map>) -> InstanceRef {
        InstanceRef {
            id: id,
            sender: sender,
            created_at: created_at,
            map: map,
        }
    }

    pub fn send(&self, command: Command) -> Result<(),()> {
        // TODO: handle errors?
        self.sender.send(command).map_err(|_| ())
    }

    pub fn get_id(&self) -> Id<Instance> {
        self.id
    }

    pub fn get_map(&self) -> Id<Map> {
        self.map
    }

    pub fn created_at(&self) -> &Tm {
        &self.created_at
    }

    pub fn get_sender(&self) -> &Sender<Command> {
        &self.sender
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let indent = "";
        try!(write!(f, "{}Instance {}:\n", indent, self.id));
        self.actors.dump(f, &self.entities)
    }
}

/// A list of things that can happen during tick calculation, which require work from the instance
pub enum TickEvent {
    EntityDeath(Entity),
}
/// Regular or delayed operations that will execute on an Instance
pub enum InstanceTick {
    /// The main operation will be calculating the next tick
    ///
    /// This will among other things execute all actions made by players
    /// since the last tick, resolve AI trees and send the update to players
    CalculateTick,
    /// This will update the Game's knowledge of all Player in this map
    UpdatePlayers,
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
            self.shutdown();
        }
    }
}

impl HasId for Instance {
    type Type = u64;
}
