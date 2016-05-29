use std::io::Read;

use byteorder::{LittleEndian, WriteBytesExt};
use lycan_serialize::{AuthenticationToken,ErrorCode};

use id::Id;
use data::Player;

pub fn generate_fake_authtok() -> Vec<(AuthenticationToken,Id<Player>)> {
    let fake_tokens = ::lycan_serialize::forge_authentication_tokens();
    fake_tokens.into_iter().map(|(u,t)| (t, Id::forge(u))).collect()
}
