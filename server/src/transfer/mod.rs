pub mod receiver;
pub mod sender;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub const WAKE_BYTE: u8 = 0xFF;

pub struct TransferSession {
    pub code: u32,
    pub sender_address: Option<SocketAddr>,
    pub proxy_stream: Option<TcpStream>,
}

pub type ActiveTransfers = Arc<Mutex<HashMap<u32, TransferSession>>>;

pub fn log_msg(msg: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let total_secs = now.as_secs();
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = (total_secs / 3600) % 24;
    let millis = now.subsec_millis();

    println!(
        "[{:02}:{:02}:{:02}.{:03}] {}",
        hours, mins, secs, millis, msg
    );
}

pub fn log_msg_with_code(code: u32, msg: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = (total_secs / 3600) % 24;
    let millis = now.subsec_millis();

    println!(
        "[{:02}:{:02}:{:02}.{:03}] [Code: {:04}] {}",
        hours, mins, secs, millis, code, msg
    );
}

pub fn handle_client(mut stream: TcpStream, transfers: ActiveTransfers) {
    let mut buffer = [0; 1024];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(0) => return,
        Ok(n) => n,
        Err(_) => return,
    };

    let get_data = String::from_utf8_lossy(&buffer[..bytes_read])
        .trim()
        .to_string();

    if let Some(code_str) = get_data.strip_prefix("GET:DIRECT:") {
        if let Ok(code) = code_str.parse::<u32>() {
            receiver::get_direct(stream, code, transfers);
            return;
        }
    }

    if let Some(code_str) = get_data.strip_prefix("GET:PROXY:") {
        if let Ok(code) = code_str.parse::<u32>() {
            receiver::get_proxy(stream, code, transfers);
            return;
        }
    }

    if let Some(code_str) = get_data.strip_prefix("DIRECT:") {
        if let Ok(code) = code_str.parse::<u32>() {
            sender::register_direct(stream, code, transfers);
            return;
        }
    }

    if let Some(code_str) = get_data.strip_prefix("PROXY:") {
        if let Ok(code) = code_str.parse::<u32>() {
            sender::register_proxy(stream, code, transfers);
        }
    }
}

pub fn relay_streams(code: u32, client1: TcpStream, client2: TcpStream) {
    let mut c1_read = client1.try_clone().expect("Failed to clone c1");
    let mut c2_write = client2.try_clone().expect("Failed to clone c2");

    let t1 = thread::spawn(move || {
        let mut buf = [0; 64 * 1024];
        while let Ok(n) = c1_read.read(&mut buf) {
            if n == 0 {
                break;
            }
            if c2_write.write_all(&buf[..n]).is_err() {
                break;
            }
        }
    });

    let mut c2_read = client2;
    let mut c1_write = client1;

    let t2 = thread::spawn(move || {
        let mut buf = [0; 64 * 1024];
        while let Ok(n) = c2_read.read(&mut buf) {
            if n == 0 {
                break;
            }
            if c1_write.write_all(&buf[..n]).is_err() {
                break;
            }
        }
    });

    let _ = t1.join();
    let _ = t2.join();
    log_msg_with_code(code, "Transfer session closed cleanly.");
}
