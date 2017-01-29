use std;
use std::net::SocketAddr;
use std::io::Read;
use std::io::Write;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::thread;
use std::sync::mpsc::Sender as StdSender;
use std::sync::mpsc::Receiver as StdReceiver;
use std::sync::mpsc::{TryRecvError};

use futures::{self,Poll,Sink,Stream};
use futures::future::{self,Loop,Future,IntoFuture,BoxFuture};
use futures::sync::mpsc::{self,UnboundedSender,UnboundedReceiver,SendError};
use tokio_core;
use tokio_core::net::TcpStream;
use tokio_core::net::TcpListener;
use tokio_core::io::{self,Io,EasyBuf,Codec};
use tokio_core::reactor::{Core,Handle};
//use tokio_core::channel::{Receiver,channel,Sender};
use bytes::RingBuf;
use byteorder::{ReadBytesExt,LittleEndian};
use uuid::Uuid;

use lycan_serialize::ErrorCode;
use lycan_serialize::Vec2d;
use lycan_serialize::Error as NetworkError;
use lycan_serialize::AuthenticationToken;

use id::Id;
use messages::{NetworkCommand,Command,NetworkGameCommand,GameCommand};
use messages::NetworkNotification;
use messages::Request;

use self::errors::{
    Error,
    ErrorKind,
    ResultExt,
};

pub struct ClientError;

mod errors {
    error_chain! {
        foreign_links {
            Io(::std::io::Error);
        }
    }
}

pub struct Client {
    pub uuid: Uuid,
    sender: UnboundedSender<InternalNotification>,
    receiver: StdReceiver<NetworkCommand>,
}

#[derive(Debug)]
enum InternalNotification {
    Disconnect,
    NetworkNotification(NetworkNotification),
}

impl From<NetworkNotification> for InternalNotification {
    fn from(n: NetworkNotification) -> InternalNotification {
        InternalNotification::NetworkNotification(n)
    }
}

impl Client {
    pub fn send(&self, notif: NetworkNotification) -> Result<(),()> {
        // TODO: Error handling
        UnboundedSender::send(&self.sender, notif.into()).map_err(|_| ())
    }

    pub fn recv(&mut self) -> Result<Option<NetworkCommand>,()> {
        match self.receiver.try_recv() {
            Ok(c) => Ok(Some(c)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(()),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = UnboundedSender::send(&self.sender, InternalNotification::Disconnect);
    }
}

impl ::std::fmt::Debug for Client {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(),::std::fmt::Error> {
        f.debug_struct("Client").field("uuid", &self.uuid).finish()
    }
}

pub fn start_server(addr: SocketAddr, tx: UnboundedSender<Request>) {
    let builder = thread::Builder::new()
        .name("Network Tokio".into());

    builder.spawn(move || {
        // Create the event loop that will drive this server
        let mut l = Core::new().unwrap();
        let handle = l.handle();

        // Create a TCP listener which will listen for incoming connections
        let socket = TcpListener::bind(&addr, &handle).unwrap();

        // Once we've got the TCP listener, inform that we have it
        println!("Listening on: {}", addr);

        let done = socket.incoming().for_each(|(socket, _addr)| {
            handle_client(socket, &handle, tx.clone());

            Ok(())
        });

        // Execute our server (modeled as a future) and wait for it to
        // complete.
        //
        // There are currently no clean way to stop the event loop, so this
        // function currently never returns
        l.run(done).unwrap();
    }).unwrap();
}

// Handles an incomming client on the network
//
// It will spawn a task on the even look associated with `handle`, that will drive
// all the network part for this client
fn handle_client(socket: TcpStream, handle: &Handle, tx: UnboundedSender<Request>) {
    let framed = socket.framed(LycanCodec::default());
    let (sink, stream) = framed.split();
    let stream = stream.and_then(|command| {
        // Log every command we receive
        trace!("Received command {:?}", command);
        Ok(command)
    }).map_err(|e| ErrorKind::Io(e).into());
    let fut = authenticate_client(stream, sink, tx)
        .and_then(|(messages, write, uuid, tx)| {
            debug!("Authenticated the client {}", uuid);
            // The connect it to the Game
            client_connected(messages, write, uuid, tx)
        }).map_err(|e| error!("Error in handle_client {}", e));

    // Finally, spawn the resulting future, so it can run in parallel of other futures
    handle.spawn(fut);
}

// XXX: We shouldn't need to box the future
// Deals with client authentication
//
// The returned future resolved to the streams given in input, plus the Uuid of the authenticated
// player
fn authenticate_client<S,W>(messages: S,
                            write: W,
                            tx: UnboundedSender<Request>)
    -> BoxFuture<(S,W,Uuid,UnboundedSender<Request>), Error>
where S: Stream<Item=NetworkCommand,Error=Error> + Send + 'static,
      W: Sink<SinkItem=NetworkNotification,SinkError=IoError> + Send + 'static
{
    let fut = future::loop_fn((messages, write, tx), move |(messages, write, tx)| {
        // This into_future() transforms the stream of message in a future that will resolve
        // to one message, and the rest of messages
        messages.into_future().map_err(|(error, messages)| {
            // TODO: This brutally drops the client ...
            drop(messages);
            error
        }).and_then(|(command, messages)| {
            match command {
                Some(NetworkCommand::GameCommand(NetworkGameCommand::Authenticate(uuid, token))) => {
                    debug!("Got authentication request {} {:?}", uuid, token);

                    let verif = verify_token(uuid, token, tx).and_then(move |(success, tx)| {
                        debug!("Authentication result: {}", success);
                        let response = if success {
                            NetworkNotification::Response { code: ErrorCode::Success }
                        } else {
                            NetworkNotification::Response { code: ErrorCode::Error }
                        };
                        write.send(response)
                            .map_err(|e| ErrorKind::Io(e).into())
                            .and_then(move |write| {
                            if success {
                                Ok(Loop::Break((messages, write, uuid, tx)))
                            } else {
                                Err("Failed authentication".into())
                            }
                        })
                    });
                    verif.into_future().boxed()
                }
                Some(other_command) => {
                    warn!("Client tried to send a command before authenticating {:?}", other_command);
                    Ok(Loop::Continue((messages, write, tx))).into_future().boxed()

                }
                None => Err("Client sent no message".into()).into_future().boxed(),
            }
        })
    });

    fut.boxed()
}

// Verifies an authentication token, returns true if the authentication was successful
fn verify_token(uuid: Uuid,
                token: AuthenticationToken,
                tx: UnboundedSender<Request>)
    -> BoxFuture<(bool,UnboundedSender<Request>),Error> {
    let (complete, oneshot) = futures::oneshot();
    let request = Request::new(move |game| {
        complete.complete(game.verify_token(Id::forge(uuid), token));
    });

    let res = UnboundedSender::send(&tx, request);
    if res.is_err() {
        Err("Game was shutdown or panicked during connection".into()).into_future().boxed()
    } else {
        oneshot
            .map(|success| (success, tx))
            .map_err(|_| "Verify token cancelled".into())
            .boxed()
    }
}

// Establishes communication between a client and the Game
//
// The returned future currently only resolves with an error, either when a communication
// problem with the client occured, or when the client disconnects
fn client_connected<S,W>(messages: S,
                         write: W,
                         uuid: Uuid,
                         sender: UnboundedSender<Request>)
    -> BoxFuture<(), Error>
where S: Stream<Item=NetworkCommand,Error=Error> + Send + 'static,
      W: Sink<SinkItem=NetworkNotification,SinkError=IoError> + Send + 'static {
    let (tx1, rx1) = std::sync::mpsc::channel();
    let (tx2, rx2) = mpsc::unbounded();

    // fut1 reads every command on the network, and sends them to the corresponding Client struct
    let fut1 = messages
        .for_each(move |message| {
            // We send every message in the channel
            // The Game or Instance can use Client::recv() to get those messages
            let res = tx1.send(message);
            if res.is_err() {
                Err("Error when sending command, client disconnected".into())
            } else {
                Ok(())
            }
        });

    // fut2 gets notifications from the Instance, and writes them on the network
    let fut2 = rx2
        .map_err(|()| "Error reading from channel".into())
        .fold(write, |write, buffer| {
            debug!("Sending notification {:?}", buffer);
            let res = match buffer {
                InternalNotification::Disconnect => {
                    Err(Error::from("The Game has disconnected the client"))
                }
                InternalNotification::NetworkNotification(n) => {
                    Ok(write.send(n).map_err(|e| Error::from(ErrorKind::Io(e))))
                }
            };
            res.into_future().flatten()
        }).map(|_| ()).boxed();

    // We run the two futures in parallel
    // XXX: This introduces potentially unwanted polling:
    // every time one of fut1 or fut2 is ready, both will be polled
    let fut = fut1.join(fut2).map(|_| ());

    // Creates the corresponding Client structure, and send it to the Game
    let client = Client {
        uuid: uuid,
        sender: tx2,
        receiver: rx1,
    };
    let request = Request::NewClient(client);
    let fut = if UnboundedSender::send(&sender, request).is_err() {
        Err(format!("Could not send client {}", uuid))
    } else {
        Ok(fut)
    };
    fut.into_future().flatten().boxed()
}

#[derive(Default,Debug,Clone,Copy)]
struct LycanCodec {
    next_msg_size: Option<usize>,
}

impl Codec for LycanCodec {
    type In = NetworkCommand;
    type Out = NetworkNotification;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<NetworkCommand>, IoError> {
        const MAX_MSG_SIZE: usize = 8 * 1024;

        let next_msg_size;

        match self.next_msg_size {
            Some(n) => {
                next_msg_size = n;
            }
            None => {
                let length = buf.len();
                if length < 8 {
                    // Not enough data to read length of next message
                    return Ok(None);
                }
                next_msg_size = buf.as_ref().read_u64::<LittleEndian>()? as usize;
                if next_msg_size > MAX_MSG_SIZE {
                    return Err(IoError::new(IoErrorKind::Other, "message too big"));
                }
                // Advance in the buffer
                let _ = buf.drain_to(8);
            }
        }

        if buf.len() >= next_msg_size {
            self.next_msg_size = None;
            let msg = buf.drain_to(next_msg_size);
            let command = NetworkCommand::deserialize(&mut msg.as_slice(), next_msg_size as u64)
                .map_err(|e| IoError::new(IoErrorKind::Other, e))?;
            Ok(Some(command))
        } else {
            self.next_msg_size = Some(next_msg_size);
            Ok(None)
        }
    }

    fn decode_eof(&mut self, buf: &mut EasyBuf) -> Result<NetworkCommand, IoError> {
        match self.decode(buf)? {
            Some(command) => Ok(command),
            None => Err(IoError::new(IoErrorKind::UnexpectedEof, "end of stream not on message boundary")),
        }
    }

    fn encode(&mut self, item: NetworkNotification, into: &mut Vec<u8>) -> Result<(), IoError> {
        item.serialize(into)
    }
}

