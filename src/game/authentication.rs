use std::io::Read;
use std::collections::HashMap;

use byteorder::{LittleEndian, WriteBytesExt};
use lycan_serialize::{AuthenticationToken,ErrorCode};

use id::Id;
use data::Player;

pub struct AuthenticationManager {
    // TODO: Timeouts
    map: HashMap<Id<Player>, AuthenticationToken>,
}

impl AuthenticationManager {
    pub fn new() -> AuthenticationManager {
        AuthenticationManager {
            map: HashMap::new(),
        }
    }

    pub fn add_token(&mut self, player: Id<Player>, token: AuthenticationToken) {
        trace!("Adding token {} for player {}", token.0, player);
        self.map.insert(player, token);
    }

    /// Verifies that the player possesses the correct authentication token
    ///
    /// Deletes the token if the authentication succeeds
    pub fn verify_token(&mut self, player: Id<Player>, token: AuthenticationToken) -> bool {
        match self.map.remove(&player) {
            Some(t) => {
                if t == token {
                    trace!("Authentication success for player {}", player);
                    true
                } else {
                    trace!("Authentication failure for player {}: invalid token", player);
                    // XXX: Is there a more efficient way than removing and re-adding?
                    self.map.insert(player, t);
                    false
                }
            }
            None => {
                trace!("Authentication failure for player {}: no associated token", player);
                false
            }
        }
    }

    /// Adds some "well-known" Id-AuthenticationToken pairs
    pub fn fake_authentication_tokens(&mut self) {
        for (uuid, token) in ::lycan_serialize::forge_authentication_tokens() {
            self.add_token(Id::forge(uuid), token);
        }
    }
}
