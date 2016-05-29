use std::fmt::{self,Formatter,Display};
use std::io::{self,Write,Error};
use std::collections::{hash_set,HashSet,HashMap};

use mio::*;

use id::{self,Id,HasId};
use entity::{Entity,EntityStore};
use messages::{self,Command,Notification,EntityOrder};
use messages::{NetworkCommand};
use network::{Client,ClientError,Message};
use actor::ActorId;

#[derive(Debug)]
pub struct NetworkActor {
    id: ActorId,
    entities: HashSet<Id<Entity>>,
    // XXX Does this belong here?
    client: Client,
    commands: CommandBuffer,
}

// A buffer of commands intented to apply some policies before adding a new command
// A policy can for example be that only one "change direction" command can be in the queue
// per Entity
//
// Currently a dumb structure that does no checks
#[derive(Default,Debug)]
struct CommandBuffer {
    commands: Vec<Command>,
    orders: Vec<EntityOrder>,
}

impl NetworkActor {
    pub fn entities_iter(&self) -> hash_set::Iter<Id<Entity>> {
        self.entities.iter()
    }

    pub fn get_id(&self) -> ActorId {
        self.id
    }

    pub fn register_entity(&mut self, entity: Id<Entity>) {
        self.entities.insert(entity);
    }

    pub fn new(id: ActorId, client: Client) -> NetworkActor {
        NetworkActor {
            id: id,
            entities: HashSet::new(),
            client: client,
            commands: Default::default(),
        }
    }

    pub fn get_commands(&mut self, other: &mut Vec<Command>) {
        self.commands.get_commands(other);
    }

    pub fn execute_orders(&mut self,
                      entities: &mut EntityStore,
                      notifications: &mut Vec<Notification>,
                      _previous: &[Notification]) {
        for order in self.commands.orders.drain(..) {
            match id::get_id_if_exists(&self.entities, order.entity) {
                None => {
                    warn!("Trying to give order to non-owned entity {}", order.entity);
                }
                Some(target) => {
                    match entities.get_mut(target) {
                        None => error!("Inconsistency entities / owned entities"),
                        Some(entity) => {
                            let res = entity.apply(order.order);
                            match res {
                                Err(_e) => {} // TODO: Send back error to network
                                Ok(Some(notif)) => notifications.push(notif),
                                Ok(None) => {}
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn dump(&self, f: &mut Formatter, indent: &str) -> Result<(), fmt::Error> {
        try!(writeln!(f, "{}Actor {} ", indent, self.id));
        // TODO
        Ok(())
    }

    /// Registers this actor to the event loop
    pub fn register<H: Handler>(&self, event_loop: &mut EventLoop<H>) -> Result<(),io::Error> {
        self.client.register(event_loop, self.id.as_token())
    }

    /// Deregisters this actor to the event loop
    pub fn deregister<H: Handler>(&self, event_loop: &mut EventLoop<H>) -> Result<(),io::Error> {
        self.client.deregister(event_loop)
    }

    pub fn ready<H: Handler>(&mut self, event_loop: &mut EventLoop<H>, event: EventSet) {
        match self.client.ready(event_loop, event, self.id.as_token()) {
            Err(ClientError::Disconnected) => {
                self.commands.push(Command::UnregisterActor(self.id));
            }
            Err(e) => {
                // IO or serialisation error
                error!("Client {} error: {:?}", self.id, e);
                self.commands.push(Command::UnregisterActor(self.id));
                if let Err(e) = self.client.deregister(event_loop) {
                    error!("Error when unregistering client: {}", e);
                }
            }
            Ok(mut messages) => {
                for message in messages.into_iter() {
                    match message {
                        NetworkCommand::GameCommand(_command) => {
                            // TODO
                            // Verify the command has a correct origin ...
                            unimplemented!();
                        }
                        NetworkCommand::EntityOrder(order) => {
                            self.commands.orders.push(order.into());
                        }
                    }
                }
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    pub fn send_message<H:Handler>(&mut self, event_loop: &mut EventLoop<H>, message: Message) {
        if let Err(e) = self.client.send_message(event_loop, message, self.id.as_token()) {
            error!("Error when sending message to client {}: {:?}", self.id, e);
            self.commands.push(Command::UnregisterActor(self.id));
            if let Err(err) = self.client.deregister(event_loop) {
                error!("Error when unregistering client: {}", err);
            }
        }
    }

    /// Queue a message, that will be sent once this actor is reregistered in an event loop
    pub fn queue_message(&mut self, message: Message) {
        self.client.queue_message(message);
    }
}

impl Display for NetworkActor {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        self.dump(f, "")
    }
}

impl CommandBuffer {
    fn push(&mut self, command: Command) {
        self.commands.push(command);
    }

    // Could have a better API
    fn get_commands(&mut self, other: &mut Vec<Command>) {
        other.append(&mut self.commands);
    }
}

impl HasId for NetworkActor {
    type Type = u64;
}
