use std::io::Write;
use std::net::TcpStream;

use crate::transfer::{ActiveTransfers, WAKE_BYTE, log_msg_with_code, relay_streams};

/// Receiver asks for the sender address to connect directly over LAN.
pub fn get_direct(mut stream: TcpStream, code: u32, transfers: ActiveTransfers) {
    let direct_address = {
        let storage = transfers.lock().unwrap();
        storage.get(&code).and_then(|session| session.sender_address)
    };

    match direct_address {
        Some(address) => {
            let instruction = format!("P2P:{}:5001\n", address.ip());
            log_msg_with_code(code, &format!("Sending transport instruction: {}", instruction.trim()));

            if stream.write_all(instruction.as_bytes()).is_ok() {
                let _ = stream.flush();
            }
        }
        None => {
            let _ = stream.write_all(b"ERROR: Invalid or expired code\n");
            log_msg_with_code(code, "Rejected DIRECT: invalid or expired code.");
        }
    }
}

/// Receiver requests a server relay: the server wakes the sender and
/// proxies the traffic.
pub fn get_proxy(mut stream: TcpStream, code: u32, transfers: ActiveTransfers) {
    let proxy_stream = {
        let mut storage = transfers.lock().unwrap();
        match storage.get_mut(&code) {
            Some(session) => session.proxy_stream.take(),
            None => None,
        }
    };

    let mut sender_stream = match proxy_stream {
        Some(s) => s,
        None => {
            let _ = stream.write_all(b"ERROR: Invalid or expired code\n");
            log_msg_with_code(code, "Rejected PROXY: invalid or expired code.");
            return;
        }
    };

    if stream.write_all(b"OK\n").is_err() {
        log_msg_with_code(code, "Failed to send transport instruction to Client 2");
        return;
    }
    let _ = stream.flush();

    if sender_stream.write_all(&[WAKE_BYTE]).is_err() {
        log_msg_with_code(code, "Failed to wake up Client 1 through the proxy bridge");
        return;
    }
    let _ = sender_stream.flush();

    log_msg_with_code(code, "Bridge established. Starting relay_streams.");
    relay_streams(code, sender_stream, stream);
}
