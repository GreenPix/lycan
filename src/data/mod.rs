use uuid::Uuid;

use id::Id;

mod map;
mod player;
mod management;
mod monster;

pub use self::map::Map;
pub use self::management::EntityManagement;
pub use self::management::EntityType;
pub use self::management::PositionInstance;
pub use self::management::PlayerStruct;
pub use self::management::MonsterStruct;
pub use self::management::SpawnMonster;
pub use self::management::ConnectCharacterParam;
pub use self::management::AuthenticatedRequest;
pub use self::player::Player;
pub use self::player::Stats;
pub use self::player::Position;
pub use self::monster::Monster;

// XXX: Hack to remove ... currently we consider only one map
lazy_static!{
    pub static ref UNIQUE_MAP: Map = {
        let uuid = Uuid::from_fields(42,42,42,&[42,42,42,42,42,42,42,42]).unwrap();
        Map::new(Id::forge(uuid))
    };
}

