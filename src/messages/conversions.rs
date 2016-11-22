use lycan_serialize::Notification as NetworkNotification;
use lycan_serialize::Order as NetworkOrder;
use lycan_serialize::EntityOrder as NetworkEntityOrder;
use lycan_serialize::Command as NetworkCommand;
use lycan_serialize::GameCommand as NetworkGameCommand;
use lycan_serialize::Direction;
use lycan_serialize::Vec2d;

use std::fmt::{self,Formatter,Debug};
use std::boxed::FnBox;

use entity::{Entity};
use game::Game;
use id::Id;
use instance::{Instance,ShuttingDownState};
use super::{GameCommand,Order,EntityOrder,Command,Notification};

/*
impl From<NetworkOrder> for Order {
    fn from(net: NetworkOrder) -> Order {
        unimplemented!();
    }
}

impl From<NetworkEntityOrder> for EntityOrder {
    fn from(net: NetworkEntityOrder) -> EntityOrder {
        unimplemented!();
    }
}


impl From<NetworkGameCommand> for GameCommand {
    fn from(net: NetworkGameCommand) -> GameCommand {
        unimplemented!();
    }
}

impl From<NetworkCommand> for Command {
    fn from(net: NetworkCommand) -> Command {
        unimplemented!();
    }
}

*/

// TODO REMOVE
impl Into<Option<NetworkNotification>> for Notification {
    fn into(self) -> Option<NetworkNotification> {
        match self {
            Notification::Walk {entity,orientation} =>
                Some(NetworkNotification::walk(entity,orientation)),
            Notification::Say{entity,message} =>
                Some(NetworkNotification::say(entity,message)),
            Notification::Position{entity,position,speed,pv} =>
                Some(NetworkNotification::position(entity,
                                                   Vec2d{x: position.x, y: position.y},
                                                   Vec2d{x: speed.x, y: speed.y},
                                                   pv)),
            Notification::ThisIsYou{entity} =>
                Some(NetworkNotification::this_is_you(entity)),
            Notification::NewEntity{entity,position,skin,pv} =>
                Some(NetworkNotification::new_entity(entity,
                                                     Vec2d{x: position.x, y: position.y},
                                                     skin,
                                                     pv)),
            Notification::EntityHasQuit{entity} => 
                Some(NetworkNotification::entity_has_quit(entity)),
            Notification::Damage{..} => {
                // XXX: Need to send that to the network
                None
            }
            Notification::Death{..} => {
                // XXX: Need to send that to the network
                None
            }
        }
    }
}
