use std::io::{self, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use crate::transport::{Cancel, SERVER_ADDR, accept_cancellable, is_cancelled, read_line};

pub fn send(code: u32, cancel: Cancel) -> io::Result<TcpStream> {
    let p2p_listener = TcpListener::bind("0.0.0.0:5001")?;
    println!("[P2P] Local listener successfully bound to 0.0.0.0:5001");

    let mut server_stream = TcpStream::connect(SERVER_ADDR)?;
    println!("[P2P] Connected to signaling server");

    let registration_msg = format!("DIRECT:{}\n", code);
    server_stream.write_all(registration_msg.as_bytes())?;
    server_stream.flush()?;

    let server_response = read_line(&mut server_stream, &cancel)?;
    drop(server_stream);

    if server_response.starts_with("ERR") {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Code conflict on server"));
    }

    println!("[P2P] Waiting for Client 2 to connect directly via LAN...");
    accept_cancellable(&p2p_listener, &cancel)
}

pub fn receive(code: u32, cancel: Cancel) -> io::Result<TcpStream> {
    let mut server_stream = TcpStream::connect(SERVER_ADDR)?;

    let request = format!("GET:DIRECT:{}\n", code);
    server_stream.write_all(request.as_bytes())?;
    server_stream.flush()?;

    let server_response = read_line(&mut server_stream, &cancel)?;
    drop(server_stream);

    if !server_response.starts_with("P2P:") {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No direct sender registered"));
    }

    let p2p_address = server_response.replace("P2P:", "");
    let target_addr: SocketAddr = p2p_address
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Bad P2P address"))?;

    if is_cancelled(&cancel) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
    }

    println!("[P2P] Connecting directly to sender at {}...", target_addr);
    TcpStream::connect_timeout(&target_addr, Duration::from_secs(5))
}
