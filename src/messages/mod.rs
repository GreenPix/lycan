pub use lycan_serialize::Notification as NetworkNotification;
pub use lycan_serialize::Order as NetworkOrder;
pub use lycan_serialize::EntityOrder as NetworkEntityOrder;
pub use lycan_serialize::Command as NetworkCommand;
pub use lycan_serialize::GameCommand as NetworkGameCommand;
pub use lycan_serialize::Direction;

// TODO REMOVE
pub use lycan_serialize::Order;
pub use lycan_serialize::EntityOrder;

use std::fmt::{self,Formatter,Debug};
use std::boxed::FnBox;

use mio::{Handler,EventLoop};
use nalgebra::{Pnt2,Vec2};

use entity::{Entity};
use game::Game;
use actor::{NetworkActor,ActorId};
use id::Id;
use instance::{Instance,ShuttingDownState};

mod conversions;

#[derive(Debug)]
pub enum Command {
    NewClient(NetworkActor,Vec<Entity>),
    Shutdown,
    Arbitrary(Arbitrary<Instance>),
    UnregisterActor(ActorId),
    AssignEntity((ActorId,Entity)),
}

impl Command {
    /// Should only be used for debugging or testing
    pub fn new<T>(closure: T) -> Command
    where T: FnOnce(&mut Instance, &mut EventLoop<Instance>),
          T: Send + 'static,
          T: Handler {
        let command = Arbitrary(Box::new(closure));
        Command::Arbitrary(command)
    }
}


pub struct Arbitrary<T: Handler>(Box<FnBox(&mut T, &mut EventLoop<T>) + Send>);

impl <T: Handler> Arbitrary<T> {
    pub fn execute(self, target: &mut T, event_loop: &mut EventLoop<T>) {
        self.0.call_box((target,event_loop));
    }
}

impl <T: Handler> Debug for Arbitrary<T> {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(),fmt::Error> {
        formatter.write_str("[arbitrary debug command]")
    }
}

#[derive(Debug)]
pub enum Request {
    Arbitrary(Arbitrary<Game>),
    UnregisteredActor {
        actor: NetworkActor,
        entities: Vec<Entity>,
    },
    InstanceShuttingDown(ShuttingDownState),
}

impl Request {
    /// Should only be used for debugging or testing
    pub fn new<T>(closure: T) -> Request
    where T: FnOnce(&mut Game, &mut EventLoop<Game>) + Send + 'static {
        let request = Arbitrary(Box::new(closure));
        Request::Arbitrary(request)
    }
}

#[derive(Debug,Clone)]
pub enum Notification {
    Walk {
        entity: u64,
        orientation: Option<Direction>,
    },
    Say {
        entity: u64,
        message: String,
    },
    Position {
        entity: u64,
        position: Pnt2<f32>,
        speed: Vec2<f32>,
        pv: u64,
    },
    ThisIsYou {
        entity: u64,
    },
    NewEntity {
        entity: u64,
        position: Pnt2<f32>,
        skin: u64,
        pv: u64,
    },
    EntityHasQuit {
        entity: u64,
    },
}

pub enum GameCommand {}

impl Notification {
    pub fn walk(id: u64, orientation: Option<Direction>) -> Notification {
        Notification::Walk {
            entity: id,
            orientation: orientation,
        }
    }

    pub fn say(id: u64, message: String) -> Notification {
        Notification::Say {
            entity: id,
            message: message,
        }
    }

    pub fn position(id: u64, position: Pnt2<f32>, speed: Vec2<f32>, pv: u64) -> Notification {
        Notification::Position {
            entity: id,
            position: position,
            speed: speed,
            pv: pv,
        }
    }

    pub fn this_is_you(id: u64) -> Notification {
        Notification::ThisIsYou {
            entity: id,
        }
    }

    pub fn new_entity(id: u64, position: Pnt2<f32>, skin: u64, pv: u64) -> Notification {
        Notification::NewEntity {
            entity: id,
            position: position,
            skin: skin,
            pv: pv,
        }
    }

    pub fn entity_has_quit(id: u64) -> Notification {
        Notification::EntityHasQuit {
            entity: id,
        }
    }
}

/*
#[derive(Debug)]
pub struct EntityOrder {
    pub target: u64,
    pub order: Order,
}

#[derive(Debug)]
pub enum Order {
}
*/

// TODO: Move to lycan-serialize
#[derive(Debug)]
pub struct EntityState {
    id: u64,
    position: Pnt2<f32>,
    orientation: Direction,
    // Skin
    // Hitbox
}

impl EntityState {
    pub fn new(id: Id<Entity>, position: Pnt2<f32>, orientation: Direction) -> EntityState {
        EntityState {
            id: id.as_u64(),
            position: position,
            orientation: orientation,
        }
    }
}
