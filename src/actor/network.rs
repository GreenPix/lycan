use std::fmt::{self,Formatter,Display};
use std::io::{self,Write,Error};
use std::collections::{hash_set,HashSet,HashMap};

use id::{self,Id,HasId};
use entity::{Entity,EntityStore};
use messages::{self,Command,Notification,EntityOrder};
use messages::{NetworkCommand};
use network::{Client,ClientError};
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

    fn receive_commands(&mut self) {
        loop {
            match self.client.recv() {
                Ok(Some(NetworkCommand::EntityOrder(order))) => {
                    self.commands.orders.push(order);
                }
                Ok(Some(NetworkCommand::GameCommand(c))) => {
                    error!("Error: the client is not supposed to send GameCommands after authentication {:?}", c);
                    self.commands.push(Command::UnregisterActor(self.id));
                    break;
                }
                Ok(None) => break,
                Err(()) => {
                    self.commands.push(Command::UnregisterActor(self.id));
                    break;
                }
            }
        }
    }

    pub fn get_commands(&mut self, other: &mut Vec<Command>) {
        self.commands.get_commands(other);
    }

    pub fn execute_orders(&mut self,
                      entities: &mut EntityStore,
                      notifications: &mut Vec<Notification>,
                      _previous: &[Notification]) {
        self.receive_commands();
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

    pub fn send_message(&mut self, message: Notification) {
        if let Err(e) = self.client.send(message.into()) {
            error!("Error when sending message to client {}: {:?}", self.client.uuid, e);
            self.commands.push(Command::UnregisterActor(self.id));
        }
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
