//use std::u64;
//use std::io::{Write,Error,Read,Cursor};
//use std::sync::Arc;
//use std::collections::VecDeque;
//use std::mem;
//use std::ops::Deref;
//use std::os::unix::io::AsRawFd;
//
//use bytes::buf::{RingBuf,Buf,MutBuf};
//use byteorder::{ReadBytesExt,ByteOrder,LittleEndian};


mod stream_adapter;
mod buffered_tcp;

use std;
use std::net::SocketAddr;
use std::io::Read;
use std::io::Write;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::thread;
use std::sync::mpsc::Sender as StdSender;
use std::sync::mpsc::Receiver as StdReceiver;
use std::sync::mpsc::{self,TryRecvError};

use futures;
use futures::{Future,IntoFuture,Poll,BoxFuture};
use futures::stream::Stream;
use tokio_core;
use tokio_core::net::TcpStream;
use tokio_core::net::TcpListener;
use tokio_core::io::{self,Io};
use tokio_core::reactor::{Core,Handle};
use tokio_core::channel::{Receiver,channel,Sender};
use bytes::RingBuf;
use byteorder::{ReadBytesExt,LittleEndian};
use uuid::Uuid;

use lycan_serialize::ErrorCode;
use lycan_serialize::Vec2d;
use messages::{NetworkCommand,Command,NetworkGameCommand,GameCommand};
use lycan_serialize::AuthenticationToken;
use messages::NetworkNotification;
use messages::Request;
use lycan_serialize::Error as NetworkError;

use self::buffered_tcp::BufferedReader;
use self::buffered_tcp::IoRef;

const DEFAULT_CAPACITY: usize = 1024;

pub struct Client {
    pub uuid: Uuid,
    sender: Sender<NetworkNotification>,
    receiver: StdReceiver<NetworkCommand>,
}

impl Client {
    pub fn send(&mut self, notif: NetworkNotification) -> Result<(),()> {
        // TODO: Error handling
        self.sender.send(notif).map_err(|_| ())
    }

    pub fn recv(&mut self) -> Result<Option<NetworkCommand>,()> {
        match self.receiver.try_recv() {
            Ok(c) => Ok(Some(c)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(()),
        }
    }
}

impl ::std::fmt::Debug for Client {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(),::std::fmt::Error> {
        f.debug_struct("Client").field("uuid", &self.uuid).finish()
    }
}

pub fn start_server(addr: SocketAddr, tx: StdSender<Request>) {
    thread::spawn(move || {
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
        l.run(done).unwrap();
    });
}

fn handle_client(socket: TcpStream, handle: &Handle, tx: StdSender<Request>) {
    let handle_clone = handle.clone();
    let fut = IoRef::new(socket).and_then(|socket| {
        let write = socket.clone();
        let read = BufferedReader::new(socket);
        let messages = stream_adapter::new_adapter(|read| {
            Some(next_message(read))
        }, read)
        .and_then(|command| {
            // Log every command we receive
            trace!("Received command {:?}", command);
            Ok(command)
        });

        let fut = authenticate_client(messages, write)
            .and_then(|(messages, write, uuid)| {
                debug!("Authenticated the client {}", uuid);
                client_connected(messages, write, uuid, handle_clone, tx)
            }).map_err(|e| error!("Error in handle_client {}", e));

        fut
    });

    handle.spawn(fut);
}

fn client_connected<S,W>(messages: S,
                         write: W,
                         uuid: Uuid,
                         handle: Handle,
                         sender: StdSender<Request>)
    -> BoxFuture<(), String>
where S: Stream<Item=NetworkCommand,Error=String> + Send + 'static,
      W: Write + Send + 'static {
    let (tx1, rx1) = std::sync::mpsc::channel();
    let (tx2, rx2) = tokio_core::channel::channel(&handle).unwrap();

    let fut1 = messages.for_each(move |message| {
        tx1.send(message).unwrap();
        Ok(())
    }).map_err(|e| format!("Error in messages {}", e));

    let fut2 = rx2.map_err(|e| format!("Error reading from channel {}", e))
        .fold(write, |write, buffer| {
            debug!("Sending notification {:?}", buffer);
            serialize_future(buffer, write)
        }).map(|_| ());

    let fut = fut1.join(fut2).map(|_| ())
      .map_err(|e| format!("Error: {}", e));

    let client = Client {
        uuid: uuid,
        sender: tx2,
        receiver: rx1,
    };
    let request = Request::NewClient(client);
    let fut = if sender.send(request).is_err() {
        Err(format!("Could not send client {}", uuid))
    } else {
        Ok(fut)
    };
    fut.into_future().flatten().boxed()
}

// XXX: We shouldn't need to box the future
fn authenticate_client<S,W>(messages: S, write: W) -> BoxFuture<(S,W,Uuid), String>
where S: Stream<Item=NetworkCommand,Error=String> + Send + 'static,
      W: Write + Send + 'static {
    let fut = messages.into_future().map_err(move |(error, _messages)| {
        // TODO: This brutally drops the client ...
        error
    }).and_then(move |(command, messages)| {
        match command {
            Some(NetworkCommand::GameCommand(NetworkGameCommand::Authenticate(uuid, token))) => {
                debug!("Got authentication request {} {:?}", uuid, token);
                Ok((messages, uuid, token))
            }
            Some(_) => Err("Client tried to send a message before authenticating".to_string()),
            None => Err("Client sent no message".to_string()),
        }
    }).and_then(move |(messages, uuid, token)| {
        verify_token(uuid, token).and_then(move |success| {
            debug!("Authentication result: {}", success);
            if !success {
                let response = NetworkNotification::Response{ code: ErrorCode::Error };
                serialize_future(response, write)
                    .and_then(move |_write| {
                        Err("Failed authentication".to_string())
                    }).boxed()
            } else {
                let response = NetworkNotification::Response{ code: ErrorCode::Success };
                serialize_future(response, write)
                    .and_then(move |write| futures::finished((messages, write, uuid)))
                    .boxed()
            }
        })
    });
    fut.boxed()
}

// TODO: unbox
fn serialize_future<W: Write + Send + 'static>(notif: NetworkNotification, writer: W) -> BoxFuture<W,String> {
    // TODO: Improve that ...
    let mut buffer = Vec::with_capacity(128);
    notif.serialize(&mut buffer).unwrap();
    io::write_all(writer, buffer)
        .map(|(writer, _buffer)| writer)
        .map_err(|e| format!("Error when writing notification {}", e))
        .boxed()
}

fn next_message<T: Io>(socket: BufferedReader<T>)
-> impl Future<Item=(BufferedReader<T>, NetworkCommand),Error=String> {
    let future = socket.ensure(8)
        .and_then(|mut socket| {
            let next_msg_size = socket.read_u64::<LittleEndian>().unwrap() as usize;
            if next_msg_size >= socket.capacity() {
                return Err(IoError::new(ErrorKind::Other,
                                        format!("The socket buffer is not big enough: next_msg_size {} capacity {}",
                                                next_msg_size,
                                                socket.capacity())));
            }
            Ok(socket.ensure(next_msg_size).map(move |s| (s, next_msg_size)))
        }).flatten().map_err(|e| e.to_string())
          .and_then(|(mut socket, next_msg_size)| {
              let command = NetworkCommand::deserialize(&mut socket, next_msg_size as u64)
                  .map(|c| (socket, c))
                  .map_err(|e| e.to_string());
              command
        });
    future
}

fn verify_token(_uuid: Uuid, _token: AuthenticationToken) -> impl Future<Item=bool,Error=String> {
    // Simulates a delay in the authentication ...
    let (complete, oneshot) = futures::oneshot();
    thread::spawn(move || {
        let duration = std::time::Duration::from_millis(100);
        thread::sleep(duration);
        complete.complete(true);
    });

    oneshot.map_err(|_| "Verify token cancelled".to_string())
}

/*
#[derive(Clone,Debug)]
pub struct Message {
    inner: Cursor<Vec<u8>>,
    size: u64,
}

impl Message {
    pub fn new(data: NetworkNotification) -> Message {
        let mut vec = Vec::new();
        data.serialize(&mut vec).unwrap();
        let size = vec.len() as u64;
        Message {
            inner: Cursor::new(vec),
            size: size,
        }
    }
}
*/

#[derive(Debug)]
pub enum ClientError {
    Disconnected,
    Socket(IoError),
    Capnp(NetworkError),
}

impl From<NetworkError> for ClientError {
    fn from(err: NetworkError) -> ClientError {
        ClientError::Capnp(err)
    }
}

impl From<IoError> for ClientError {
    fn from(err: IoError) -> ClientError {
        ClientError::Socket(err)
    }
}
