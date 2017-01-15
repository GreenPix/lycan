use lycan_serialize::{
    Notification as NetworkNotification,
    Order as NetworkOrder,
    EntityOrder as NetworkEntityOrder,
    Command as NetworkCommand,
    GameCommand as NetworkGameCommand,
    EntityUpdate as NetworkEntityUpdate,
    Direction,
    Vec2d,
};

use std::fmt::{self,Formatter,Debug};
use std::boxed::FnBox;

use entity::{Entity};
use game::Game;
use id::Id;
use instance::{Instance,ShuttingDownState};
use super::{
    GameCommand,
    Order,
    EntityOrder,
    Command,
    Notification,
    EntityUpdate,
};

// Conversion between internal notifications and notifications sent on the network
// Is this separation always needed?
//
// Return an option, in case the network part is not up to date (so we don't break clients)
impl Into<Option<NetworkNotification>> for Notification {
    fn into(self) -> Option<NetworkNotification> {
        match self {
            Notification::Walk {entity,orientation} =>
                Some(NetworkNotification::walk(entity,orientation)),
            Notification::Say{entity,message} =>
                Some(NetworkNotification::say(entity,message)),
            Notification::GameUpdate{tick_id,entities} =>
                Some(NetworkNotification::GameUpdate {
                    tick_id: tick_id,
                    entities: entities.into_iter().map(|e| e.into()).collect(),
                }),
            Notification::ThisIsYou{entity} =>
                Some(NetworkNotification::this_is_you(entity)),
            Notification::NewEntity{entity,position,skin,pv,nominal_speed} =>
                Some(NetworkNotification::new_entity(entity,
                                                     Vec2d{x: position.x, y: position.y},
                                                     skin,
                                                     pv,
                                                     nominal_speed)),
            Notification::EntityHasQuit{entity} =>
                Some(NetworkNotification::entity_has_quit(entity)),
            Notification::Damage{source, victim, amount} => {
                Some(NetworkNotification::Damage {
                    source: source,
                    victim: victim,
                    amount: amount,
                })
            }
            Notification::Death{entity} => {
                Some(NetworkNotification::Death {
                    entity: entity,
                })
            }
        }
    }
}

impl Into<NetworkEntityUpdate> for EntityUpdate {
    fn into(self) -> NetworkEntityUpdate {
        // Destructuration so we don't forget to send some things to the network
        let EntityUpdate {
            entity_id,
            position,
            speed,
            pv,
        } = self;
        NetworkEntityUpdate {
            entity_id: entity_id,
            position: Vec2d { x: position.x, y: position.y },
            speed: Vec2d { x: speed.x, y: speed.y },
            pv: pv,
        }
    }
}
