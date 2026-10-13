pub mod manager;

pub mod p2p_direct;
pub mod p2p_radar;
pub mod p2p_proxy;

use std::io::{self, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Shared kill-switch: once one transport wins, the rest must stop.
pub type Cancel = Arc<AtomicBool>;

/// Address clients use to reach the server (build-time config).
pub const SERVER_ADDR: &str = env!("SERVER_ADDR");

pub fn new_cancel() -> Cancel {
    Arc::new(AtomicBool::new(false))
}

pub fn is_cancelled(cancel: &Cancel) -> bool {
    cancel.load(Ordering::SeqCst)
}

pub fn read_line(stream: &mut TcpStream, cancel: &Cancel) -> io::Result<String> {
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;

    let mut buf: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        if is_cancelled(cancel) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }

        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                buf.push(byte[0]);
                if buf.len() > 128 {
                    break;
                }
            }
            Err(ref e)
            if e.kind() == io::ErrorKind::WouldBlock
                || e.kind() == io::ErrorKind::TimedOut =>
                {
                    continue;
                }
            Err(e) => return Err(e),
        }
    }

    stream.set_read_timeout(None)?;
    Ok(String::from_utf8_lossy(&buf).trim().to_string())
}

pub fn accept_cancellable(listener: &TcpListener, cancel: &Cancel) -> io::Result<TcpStream> {
    listener.set_nonblocking(true)?;

    loop {
        if is_cancelled(cancel) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }

        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}
