use std::u64;
use std::io::{Write,Error,Read,Cursor};
use std::sync::Arc;
use std::collections::VecDeque;
use std::mem;
use std::ops::Deref;
use std::os::unix::io::AsRawFd;

use mio::*;
use bytes::buf::{RingBuf,Buf,MutBuf};
use mio::tcp::TcpStream;
use mio::unix::EventedFd;
use byteorder::{ReadBytesExt,ByteOrder,LittleEndian};

use smallvec::SmallVec;
use messages::{NetworkCommand,Command};
use lycan_serialize::Error as NetworkError;
use messages::NetworkNotification;

const DEFAULT_CAPACITY: usize = 1024;

// TODO: We need to handle SSL ourselves
#[derive(Debug)]
pub struct Client {
    interest: EventSet,
    socket: TcpStream,
    read_buffer: RingBuf,
    next_msg_size: Option<usize>,
    connection_lost: bool,
    message_queue: VecDeque<Message>,
}

impl Client {
    pub fn new(socket: TcpStream) -> Client {
        Client {
            socket: socket,
            interest: EventSet::readable() | EventSet::error() | EventSet::hup(),
            read_buffer: RingBuf::new(DEFAULT_CAPACITY),
            next_msg_size: None,
            connection_lost: false,
            message_queue: VecDeque::new(),
        }
    }

    // TODO: potentially collapse ready, writable and readable
    pub fn ready<H: Handler>(&mut self,
                             event_loop: &mut EventLoop<H>,
                             event: EventSet,
                             token: Token)
    -> Result<SmallVec<[NetworkCommand;4]>,ClientError> {
        if event.is_hup() || event.is_error() {
            if event.is_error() {
                error!("Transmission error for client {}", token.as_usize());
            }
            // The client disconnected his side, we need to disconnect him
            debug!("Disconnecting client {}", token.as_usize());
            self.connection_lost = true;
            try!(event_loop.deregister(&self.socket));
            return Err(ClientError::Disconnected);
        }
        if event.is_writable() {
            try!(self.writable(event_loop, token));
        }
        let vec = if event.is_readable() {
            try!(self.readable(event_loop, token))
        } else {
            SmallVec::new()
        };
        Ok(vec)
    }

    fn writable<H: Handler>(&mut self, event_loop: &mut EventLoop<H>, token: Token)
        -> Result<(),ClientError> {
        while let Some(mut message) = self.message_queue.pop_front() {
            match try!(self.socket.try_write_buf(&mut message.inner)) {
                None => {
                    // We cannot write any more, replace the message in the front of the queue
                    self.message_queue.push_front(message);
                    break;
                }
                Some(size_written) => {
                    trace!("Wrote {} bytes to the socket {}", size_written, token.as_usize());
                    // Check if the message has only been partially sent. If yes put it back
                    if message.inner.position() < message.size {
                        self.message_queue.push_front(message);
                        break;
                    }
                }
            }
        }
        if self.message_queue.is_empty() {
            self.interest.remove(EventSet::writable());
        }
        try!(event_loop.reregister(&self.socket,
                                   token,
                                   self.interest,
                                   PollOpt::level()));
        Ok(())
    }

    fn readable<H: Handler>(&mut self,
                            event_loop: &mut EventLoop<H>,
                            token: Token) 
    -> Result<SmallVec<[NetworkCommand;4]>,ClientError> {
        // TODO: Throttling

        let mut retry = true;
        let mut res = SmallVec::new();
        while retry {
            match try!(self.socket.try_read_buf(&mut self.read_buffer)) {
                None => {
                    try!(event_loop.reregister(&self.socket, token, self.interest, PollOpt::level()));
                    return Ok(res);
                }
                Some(size_read) => {
                    trace!("Read {} bytes from the socket", size_read);

                    // Sanity check ... this should not happen here
                    if size_read == 0 {
                        error!("Bad state for client {}, disconnecting him", token.as_usize());
                        self.connection_lost = true;
                        try!(event_loop.deregister(&self.socket));
                        return Err(ClientError::Disconnected);
                    }

                    if MutBuf::has_remaining(&self.read_buffer) {
                        // The OS did not fill our buffer, there must not be any data left
                        retry = false;
                    }

                    try!(self.handle_data(&mut res));
                }
            }
        }
        Ok(res)
    }

    fn handle_data(&mut self, res: &mut SmallVec<[NetworkCommand;4]>)
        -> Result<(),ClientError> {
        let size_u64 = 8;
        let mut next_msg_size = match self.next_msg_size.take() {
            Some(number) => {
                trace!("Saved message size: {}", number);
                number
            }
            None => {
                // XXX: Manage the error case more gracefully
                if Buf::remaining(&self.read_buffer) < size_u64 {
                    // We wait for more data
                    trace!("Not enough data to read next_msg_size");
                    return Ok(());
                }
                let next_msg_size = self.read_buffer.read_u64::<LittleEndian>().unwrap() as usize;
                if next_msg_size > self.read_buffer.capacity() {
                    error!("Next message will be too big: {}", next_msg_size);
                    // TODO TODO TODO
                    unimplemented!();
                }
                trace!("Newly read message size: {}", next_msg_size);
                next_msg_size
            }
        };

        while next_msg_size <= Buf::remaining(&self.read_buffer) {
            let command = try!(NetworkCommand::deserialize(&mut self.read_buffer, next_msg_size as u64));
            res.push(command);

            if Buf::remaining(&self.read_buffer) < size_u64 {
                trace!("Loop: Not enough data to read next_msg_size");
                return Ok(());
            }
            next_msg_size = self.read_buffer.read_u64::<LittleEndian>().unwrap() as usize;
            if next_msg_size > self.read_buffer.capacity() {
                error!("Next message will be too big");
                // TODO TODO TODO
                unimplemented!();
            }
            trace!("Loop: Next message size: {}", next_msg_size);
        }
        trace!("Saving next_msg_size: {}", next_msg_size);
        self.next_msg_size = Some(next_msg_size);
        Ok(())
    }

    pub fn send_message<H:Handler>(&mut self, event_loop: &mut EventLoop<H>, message: Message, token: Token)
    -> Result<(),ClientError> {
        self.message_queue.push_back(message);
        self.interest.insert(EventSet::writable());
        self.writable(event_loop, token)
    }

    /// Queue a message, that will be sent once this actor is reregistered in an event loop
    pub fn queue_message(&mut self, message: Message) {
        self.message_queue.push_back(message);
        self.interest.insert(EventSet::writable());
    }

    pub fn register<H: Handler>(&self, event_loop: &mut EventLoop<H>, token: Token)
        -> Result<(),Error> {
            // Bypass a Windows limitation that is artificially enforced on *nix systems by mio
            // See https://github.com/carllerche/mio/issues/308
            let raw_fd = self.socket.as_raw_fd();
            let evented_fd = EventedFd(&raw_fd);
            event_loop.register(&evented_fd,
                                    token,
                                    self.interest,
                                    PollOpt::level())
    }

    pub fn deregister<H: Handler>(&self, event_loop: &mut EventLoop<H>) -> Result<(),Error> {
        if !self.connection_lost {
            event_loop.deregister(&self.socket)
        } else {
            Ok(())
        }
    }

    pub fn is_connected(&self) -> bool {
        !self.connection_lost
    }

    pub fn close_read<H: Handler>(&mut self, event_loop: &mut EventLoop<H>, token: Token)
        -> Result<(),Error> {
        self.interest.remove(EventSet::readable());
        event_loop.register(&self.socket,
                                token,
                                self.interest,
                                PollOpt::level())
    }

    pub fn open_read<H: Handler>(&mut self, event_loop: &mut EventLoop<H>, token: Token)
        -> Result<(),Error> {
        self.interest.insert(EventSet::readable());
        event_loop.register(&self.socket,
                                token,
                                self.interest,
                                PollOpt::level())
    }
}

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

#[derive(Debug)]
pub enum ClientError {
    Disconnected,
    Socket(Error),
    Capnp(NetworkError),
}

impl From<NetworkError> for ClientError {
    fn from(err: NetworkError) -> ClientError {
        ClientError::Capnp(err)
    }
}

impl From<Error> for ClientError {
    fn from(err: Error) -> ClientError {
        ClientError::Socket(err)
    }
}
