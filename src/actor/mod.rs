mod network;
mod mob;

use id::Id;

pub use self::network::NetworkActor;
pub use self::mob::AiActor;

pub type ActorId = Id<NetworkActor>;
