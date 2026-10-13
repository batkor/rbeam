use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

pub mod transfer;

const SERVER_BIND: &str = env!("SERVER_BIND");

fn main() {
    let listener = TcpListener::bind(SERVER_BIND).expect("Failed to bind server socket");
    println!("Relay server is running and listening on {}", SERVER_BIND);

    let global_storage: transfer::ActiveTransfers = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let storage_reference = Arc::clone(&global_storage);

                thread::spawn(move || {
                    transfer::handle_client(stream, storage_reference);
                });
            }
            Err(e) => {
                eprintln!("Incoming connection error: {}", e);
            }
        }
    }
}
