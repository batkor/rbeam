use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::transfer::{ActiveTransfers, TransferSession, log_msg, log_msg_with_code};

pub fn register_direct(mut stream: TcpStream, code: u32, transfers: ActiveTransfers) {
    let sender_address = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            log_msg(&format!("Failed to get peer address: {}", e));
            return;
        }
    };

    let mut storage = transfers.lock().unwrap();

    let session = storage.entry(code).or_insert_with(|| TransferSession {
        code,
        sender_address: None,
        proxy_stream: None,
    });

    if session.sender_address.is_some() {
        log_msg_with_code(code, "DIRECT registration failed: Code conflict");
        let _ = stream.write_all(b"ERR:CONFLICT\n");
        return;
    }

    if stream.write_all(b"OK\n").is_err() {
        return;
    }

    session.sender_address = Some(sender_address);
    log_msg_with_code(code, "Registered DIRECT sender from Client 1");
    drop(storage);

    schedule_expiry(code, transfers, true);
}

pub fn register_proxy(mut stream: TcpStream, code: u32, transfers: ActiveTransfers) {
    let mut storage = transfers.lock().unwrap();

    let session = storage.entry(code).or_insert_with(|| TransferSession {
        code,
        sender_address: None,
        proxy_stream: None,
    });

    if session.proxy_stream.is_some() {
        log_msg_with_code(code, "PROXY registration failed: Code conflict");
        let _ = stream.write_all(b"ERR:CONFLICT\n");
        return;
    }

    if stream.write_all(b"OK\n").is_err() {
        return;
    }

    session.proxy_stream = Some(stream);
    log_msg_with_code(code, "Registered PROXY bridge from Client 1");
    drop(storage);

    schedule_expiry(code, transfers, true);
}

fn schedule_expiry(code: u32, transfers: ActiveTransfers, is_proxy: bool) {
    let timeout_storage = Arc::clone(&transfers);

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(300));

        let mut storage = timeout_storage.lock().unwrap();

        if let Some(session) = storage.get(&code) {
            let still_registered = if is_proxy {
                session.proxy_stream.is_some()
            } else {
                session.sender_address.is_some()
            };

            if still_registered {
                storage.remove(&code);
                log_msg_with_code(code, "Session expired and was auto-deleted.");
            }
        }
    });
}
