use std::io::{self};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::Duration;

use crate::transport::{accept_cancellable, is_cancelled, Cancel};

pub fn send(code: u32, cancel: Cancel) -> io::Result<TcpStream> {
    let p2p_listener = TcpListener::bind("0.0.0.0:0")?;
    let assigned_tcp_port = p2p_listener.local_addr()?.port();

    println!(
        "[Radar Sender] Local TCP listener bound to dynamic port: {}",
        assigned_tcp_port
    );

    let broadcaster_cancel = cancel.clone();
    thread::spawn(move || {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(_) => return,
        };
        if socket.set_broadcast(true).is_err() {
            return;
        }

        let msg = format!("RADAR_ANNOUNCE:{}:{}", code, assigned_tcp_port);
        let broadcast_addr: SocketAddr = "255.255.255.255:5002".parse().unwrap();

        while !is_cancelled(&broadcaster_cancel) {
            if socket.send_to(msg.as_bytes(), &broadcast_addr).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
    });

    accept_cancellable(&p2p_listener, &cancel)
}

pub fn receive(code: u32, cancel: Cancel) -> io::Result<TcpStream> {
    let socket = UdpSocket::bind("0.0.0.0:5002")?;
    socket.set_read_timeout(Some(Duration::from_millis(300)))?;

    let mut buffer = [0; 64];
    let prefix = format!("RADAR_ANNOUNCE:{}:", code);

    loop {
        if is_cancelled(&cancel) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "radar cancelled",
            ));
        }

        let (bytes_read, peer_addr) = match socket.recv_from(&mut buffer) {
            Ok(res) => res,
            Err(_) => continue,
        };

        let incoming_msg = String::from_utf8_lossy(&buffer[..bytes_read]);
        let trimmed_msg = incoming_msg.trim();

        if trimmed_msg.starts_with(&prefix) {
            let port_str = trimmed_msg.replace(&prefix, "");
            let target_port = match port_str.parse::<u16>() {
                Ok(p) => p,
                Err(_) => continue,
            };

            println!(
                "[Radar Log] Peer discovered via UDP Radar at: {}",
                peer_addr.ip()
            );

            let target_tcp_addr = SocketAddr::new(peer_addr.ip(), target_port);
            if let Ok(stream) = TcpStream::connect_timeout(&target_tcp_addr, Duration::from_secs(3))
            {
                return Ok(stream);
            }
        }
    }
}
