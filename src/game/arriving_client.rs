// Clients that have just arrived on the server

use std::net::{SocketAddr};
use std::collections::hash_map::{HashMap,Entry};

use mio::*;
use bytes::buf::{RingBuf,Buf,MutBuf};
use mio::tcp::TcpStream;
use lycan_serialize::{AuthenticationToken,ErrorCode};
use smallvec::SmallVec;

use actor::{NetworkActor};
use id::{Id,ConvertTo};
use data::{Map,Player};
use instance::Instance;
use network::*;
use super::authentication;
use messages::{NetworkNotification,NetworkGameCommand,NetworkCommand};

impl ConvertTo<NetworkActor> for ArrivingClient {}

#[derive(Debug)]
pub struct ArrivingClientManager {
    clients: HashMap<Id<ArrivingClient>, ArrivingClient>,
    tokens: HashMap<Id<Player>, AuthenticationToken>,
}

impl ArrivingClientManager {
    pub fn new() -> ArrivingClientManager {
        ArrivingClientManager {
            clients: HashMap::new(),
            tokens: HashMap::new(),
        }
    }

    pub fn new_client<H: Handler>(&mut self,
                                  socket: TcpStream,
                                  event_loop: &mut EventLoop<H>) {
        let mut client = ArrivingClient::new(socket);
        let id = client.get_id();

        // XXX: Hack
        client.set_udp_addr("0.0.0.0:0".parse().unwrap());
        if client.register(event_loop) {
            self.clients.insert(id, client);
        } else {
            error!("Failed to register client {}, dropping him", id);
        }
    }

    pub fn new_auth_tok(&mut self, tok: AuthenticationToken, id: Id<Player>) {
        self.tokens.insert(id, tok);
    }

    pub fn ready<H: Handler>(&mut self,
                             event_loop: &mut EventLoop<H>,
                             token: Token,
                             hint: EventSet)
    -> Option<(NetworkActor, Id<Player>)> {
        let id;
        let mut ret;
        let id_u64 = token.as_usize() as u64;
        match self.clients.get_mut(&id_u64) {
            Some(client) => {
                id = client.get_id();
                ret = client.ready(event_loop, hint);
            }
            _ => {
                error!("Calling ready on client {}, but no client in the hashmap",
                       token.as_usize());
                return None;
            }
        }

        for action in ret.into_iter() {
            match action {
                ArrivingClientAction::Remove => {
                    self.clients.remove(&id_u64);
                    return None;
                }
                ArrivingClientAction::VerifyToken(player_id, auth_token) => {
                    match self.tokens.remove(&player_id) {
                        None => {
                            error!("No corresponding token for client {}", token.as_usize());
                            // TODO: Insert a number of retries
                            let mut client = self.clients.get_mut(&id_u64).unwrap();
                            let response = NetworkNotification::response(ErrorCode::Error);
                            client.send_message(event_loop, Message::new(response));
                        }
                        Some(token) => {
                            if token == auth_token {
                                trace!("Token verified for client {}", id_u64);
                                let mut entry = match self.clients.entry(id) {
                                    Entry::Occupied(e) => e,
                                    _ => unreachable!(),
                                };
                                let id = Id::forge(player_id);
                                if entry.get_mut().set_player_id(id) {
                                    // The client is ready, we can return him
                                    trace!("Sending response to client {}", id_u64);
                                    let response = NetworkNotification::response(ErrorCode::Success);
                                    entry.get_mut().send_message(event_loop, Message::new(response));
                                    let client = entry.remove();
                                    client.deregister(event_loop);
                                    return Some((client.into(), id));
                                }
                            } else {
                                warn!("Invalid token for player {}", id);
                                let mut client = self.clients.get_mut(&id_u64).unwrap();
                                let response = NetworkNotification::response(ErrorCode::Error);
                                client.send_message(event_loop, Message::new(response));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}


#[derive(Debug)]
struct ArrivingClient {
    id: Id<ArrivingClient>,
    client: Client,
    authenticated: Option<Id<Player>>,
    udp_addr: Option<SocketAddr>,
}

impl ArrivingClient {
    fn new(socket: TcpStream) -> ArrivingClient {
        ArrivingClient {
            id: Id::new(),
            client: Client::new(socket),
            authenticated: None,
            udp_addr: None,
        }
    }

    fn get_id(&self) -> Id<Self> {
        self.id
    }

    fn ready<H: Handler>(&mut self,
                                event_loop: &mut EventLoop<H>,
                                hint: EventSet)
    -> SmallVec<[ArrivingClientAction;4]> {
        let mut res = SmallVec::new();
        match self.client.ready(event_loop, hint, self.id.as_token()) {
            Err(ClientError::Disconnected) => {
                res.push(ArrivingClientAction::Remove)
            }
            Err(e) => {
                // IO or serialisation error
                error!("Client {} error: {:?}", self.id, e);
                self.deregister(event_loop);
                res.push(ArrivingClientAction::Remove)
            }
            Ok(mut messages) => {
                for message in messages.into_iter() {
                    match message {
                        NetworkCommand::GameCommand(
                            NetworkGameCommand::Authenticate(id_player, authentication_token)) => {
                            trace!("Pushing authentication token");
                            res.push(ArrivingClientAction::VerifyToken(id_player, authentication_token));
                        }
                        _ => {
                            warn!("Client {} tried to send a command before authenticating",
                                  self.id);
                        }
                    }
                }
            }
        }
        res
    }

    fn send_message<H:Handler>(&mut self,
                                event_loop: &mut EventLoop<H>,
                                message: Message)
    -> Option<ArrivingClientAction> {
        let id = self.id;
        if let Err(e) = self.client.send_message(event_loop, message, id.as_token()) {
            error!("Error when sending message to client {}: {:?}", id, e);
            self.deregister(event_loop);
            Some(ArrivingClientAction::Remove)
        } else {
            None
        }
    }

    fn deregister<H:Handler>(&self, event_loop: &mut EventLoop<H>) {
        if let Err(err) = self.client.deregister(event_loop) {
            error!("Error when unregistering client: {}", err);
        }
    }

    fn register<H:Handler>(&self, event_loop: &mut EventLoop<H>) -> bool {
        let token = self.id.as_token();
        if let Err(err) = self.client.register(event_loop, token) {
            error!("Error when registering client: {}", err);
            false
        } else {
            true
        }
    }

    fn set_player_id(&mut self, id: Id<Player>) -> bool {
        self.authenticated = Some(id);
        self.udp_addr.is_some()
    }

    fn set_udp_addr(&mut self, udp_addr: SocketAddr) -> bool {
        self.udp_addr = Some(udp_addr);
        self.authenticated.is_some()
    }
}

// I did not want to put that in Requests ...
#[derive(Debug)]
enum ArrivingClientAction {
    Remove,
    VerifyToken(u64, AuthenticationToken),
}

#[derive(Debug)]
enum CurrentState {
    JustArrived,
    Authenticated(Id<Player>),
    UdpConnectionEstablished,
}

impl Into<NetworkActor> for ArrivingClient {
    fn into(self) -> NetworkActor {
        NetworkActor::new(self.id.convert(), self.client)
    }
}
