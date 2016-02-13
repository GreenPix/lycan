use std::io::Read;

use byteorder::{LittleEndian, WriteBytesExt};
use lycan_serialize::{AuthenticationToken,ErrorCode};

use id::Id;
use data::Player;

pub fn generate_fake_authtok() -> Vec<(AuthenticationToken,Id<Player>)> {
    (0..30).map(|i| {
        let token = AuthenticationToken::new(i);
        let id = Id::forge(i);
        (token, id)
    }).collect()
}
