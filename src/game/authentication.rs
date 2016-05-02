use std::io::Read;

use byteorder::{LittleEndian, WriteBytesExt};
use lycan_serialize::{AuthenticationToken,ErrorCode};

use id::Id;
use data::Player;

pub fn generate_fake_authtok() -> Vec<(AuthenticationToken,Id<Player>)> {
    (0..30).map(|i| (AuthenticationToken(i.to_string()), Id::forge(i))).collect()
}
