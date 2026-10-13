use std::io;
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use rand::Rng;
use crate::protocol::ReceiveConfig;
use crate::transport::{new_cancel, p2p_direct, p2p_proxy, p2p_radar};

enum SelectedTransport {
    Radar(TcpStream),
    Direct(TcpStream),
    Proxy(TcpStream),
}

pub fn send(path: &String, selected_transport: &str) -> io::Result<()> {
    println!("=== Starting transport send manager ===");

    let mut rng = rand::thread_rng();
    let code: u32 = rng.gen_range(100000..=999999);
    println!("Your secure file-sharing code is: {}", code);

    let target_path = crate::utils::get_realpath(path)?;
    let cancel = new_cancel();
    let (tx, rx) = mpsc::channel::<SelectedTransport>();

    if enabled(selected_transport, "radar") {
        let c = cancel.clone();
        let radar_tx = tx.clone();
        thread::spawn(move || {
            if let Ok(s) = p2p_radar::send(code, c) {
                let _ = radar_tx.send(SelectedTransport::Radar(s));
            }
        });
    }

    if enabled(selected_transport, "direct") {
        let c = cancel.clone();
        let direct_tx = tx.clone();
        thread::spawn(move || {
            if let Ok(s) = p2p_direct::send(code, c) {
                let _ = direct_tx.send(SelectedTransport::Direct(s));
            }
        });
    }

    if enabled(selected_transport, "proxy") {
        let c = cancel.clone();
        let proxy_tx = tx.clone();
        thread::spawn(move || {
            if let Ok(s) = p2p_proxy::send(code, c) {
                let _ = proxy_tx.send(SelectedTransport::Proxy(s));
            }
        });
    }

    drop(tx);

    let (mut active_stream, transport_name) = match rx.recv_timeout(Duration::from_secs(300)) {
        Ok(SelectedTransport::Radar(s)) => (s, "RADAR"),
        Ok(SelectedTransport::Direct(s)) => (s, "DIRECT"),
        Ok(SelectedTransport::Proxy(s)) => (s, "PROXY"),
        Err(_) => {
            cancel.store(true, Ordering::SeqCst);
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "All sending transport channels failed"
            ));
        }
    };

    cancel.store(true, Ordering::SeqCst);
    drop(rx);

    println!("Receiver connected via transport: [{}]. Starting directory crawler...", transport_name);

    if let Err(e) = crate::path_crawler::send_path(target_path, &mut active_stream) {
        eprintln!("[Manager Log] Directory crawler failed: {}", e);
        return Err(e);
    }

    println!("[Manager Log] Transfer completely finished. Exiting.");
    Ok(())
}

fn enabled(selected: &str, name: &str) -> bool {
    selected.eq_ignore_ascii_case("all") || selected.eq_ignore_ascii_case(name)
}

pub fn receive(code: u32, config: &ReceiveConfig) -> io::Result<()> {
    println!("=== Starting transport receive manager ===");

    let cancel = new_cancel();
    let (tx, rx) = mpsc::channel::<SelectedTransport>();

    let c = cancel.clone();
    let radar_tx = tx.clone();
    thread::spawn(move || {
        if let Ok(s) = p2p_radar::receive(code, c) {
            let _ = radar_tx.send(SelectedTransport::Radar(s));
        }
    });

    let c = cancel.clone();
    let direct_tx = tx.clone();
    thread::spawn(move || {
        if let Ok(s) = p2p_direct::receive(code, c) {
            let _ = direct_tx.send(SelectedTransport::Direct(s));
        }
    });

    let c = cancel.clone();
    let proxy_tx = tx.clone();
    thread::spawn(move || {
        if let Ok(s) = p2p_proxy::connect(code, c) {
            let _ = proxy_tx.send(SelectedTransport::Proxy(s));
        }
    });

    drop(tx);

    let (active_stream, mode_label) = match rx.recv_timeout(Duration::from_secs(300)) {
        Ok(SelectedTransport::Radar(s)) => (s, "RADAR"),
        Ok(SelectedTransport::Direct(s)) => (s, "DIRECT"),
        Ok(SelectedTransport::Proxy(s)) => (s, "PROXY"),
        Err(_) => {
            cancel.store(true, Ordering::SeqCst);
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "All parallel transport channels failed"
            ));
        }
    };

    cancel.store(true, Ordering::SeqCst);
    drop(rx);

    println!("Match found! Connected to Sender. Starting packet parser...");
    p2p_proxy::receive(active_stream, mode_label, config)?;

    Ok(())
}
