use lycan_serialize::Notification as NetworkNotification;
use lycan_serialize::Order as NetworkOrder;
use lycan_serialize::EntityOrder as NetworkEntityOrder;
use lycan_serialize::Command as NetworkCommand;
use lycan_serialize::GameCommand as NetworkGameCommand;
use lycan_serialize::Direction;
use lycan_serialize::Vec2d;

use std::fmt::{self,Formatter,Debug};
use std::boxed::FnBox;

use mio::{Handler,EventLoop};

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
impl From<Notification> for NetworkNotification {
    fn from(notif: Notification) -> NetworkNotification {
        match notif {
            Notification::Walk {entity,orientation} =>
                NetworkNotification::walk(entity,orientation),
            Notification::Say{entity,message} =>
                NetworkNotification::say(entity,message),
            Notification::Position{entity,position,speed,pv} =>
                NetworkNotification::position(entity,
                                              Vec2d{x: position.x, y: position.y},
                                              Vec2d{x: speed.x, y: speed.y},
                                              pv),
            Notification::ThisIsYou{entity} =>
                NetworkNotification::this_is_you(entity),
            Notification::NewEntity{entity,position,skin,pv} =>
                NetworkNotification::new_entity(entity,
                                                Vec2d{x: position.x, y: position.y},
                                                skin,
                                                pv),
            Notification::EntityHasQuit{entity} => 
                NetworkNotification::entity_has_quit(entity),
        }
    }
}
