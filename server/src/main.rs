use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

pub mod transfer;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:80").expect("Failed to bind to port 80");
    println!("Relay server is running and listening on 0.0.0.0:80");

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
