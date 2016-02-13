use std::net::{TcpStream,TcpListener};
use std::thread;

pub fn create_connection() -> (TcpStream,TcpStream) {
    let server = TcpListener::bind("localhost:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let guard = thread::scoped(|| {
        TcpStream::connect(("localhost",port)).unwrap()
    });
    let (server_socket, _) = server.accept().unwrap();
    let client_socket = guard.join();
    (server_socket, client_socket)
}
