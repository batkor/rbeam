use crate::protocol::ReceiveConfig;
use crate::transport::{new_cancel, p2p_direct, p2p_proxy, p2p_radar};
use crate::utils::SessionCode;
use std::io;
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

enum SelectedTransport {
    Radar(TcpStream),
    Direct(TcpStream),
    Proxy(TcpStream),
}

pub fn send(path: &String, selected_transport: &str) -> io::Result<()> {
    println!("=== Starting transport send manager ===");

    let session = crate::utils::generate_session_code();
    let nameplate = session.nameplate;
    println!("Your secure file-sharing code is: {}", session.display());

    let target_path = crate::utils::get_realpath(path)?;
    let cancel = new_cancel();
    let (tx, rx) = mpsc::channel::<SelectedTransport>();

    if enabled(selected_transport, "radar") {
        let c = cancel.clone();
        let radar_tx = tx.clone();
        thread::spawn(move || {
            if let Ok(s) = p2p_radar::send(nameplate, c) {
                let _ = radar_tx.send(SelectedTransport::Radar(s));
            }
        });
    }

    if enabled(selected_transport, "direct") {
        let c = cancel.clone();
        let direct_tx = tx.clone();
        thread::spawn(move || {
            if let Ok(s) = p2p_direct::send(nameplate, c) {
                let _ = direct_tx.send(SelectedTransport::Direct(s));
            }
        });
    }

    if enabled(selected_transport, "proxy") {
        let c = cancel.clone();
        let proxy_tx = tx.clone();
        thread::spawn(move || {
            if let Ok(s) = p2p_proxy::send(nameplate, c) {
                let _ = proxy_tx.send(SelectedTransport::Proxy(s));
            }
        });
    }

    drop(tx);

    let (active_stream, transport_name) = match rx.recv_timeout(Duration::from_secs(300)) {
        Ok(SelectedTransport::Radar(s)) => (s, "RADAR"),
        Ok(SelectedTransport::Direct(s)) => (s, "DIRECT"),
        Ok(SelectedTransport::Proxy(s)) => (s, "PROXY"),
        Err(_) => {
            cancel.store(true, Ordering::SeqCst);
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "All sending transport channels failed",
            ));
        }
    };

    cancel.store(true, Ordering::SeqCst);
    drop(rx);

    active_stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    active_stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
    let mut secure = crate::crypto::handshake_initiator(active_stream, session.secret.as_bytes())?;
    secure.get_mut().set_read_timeout(None)?;
    secure.get_mut().set_write_timeout(None)?;

    println!(
        "Secure channel established via [{}]. Starting directory crawler...",
        transport_name
    );

    if let Err(e) = crate::path_crawler::send_path(target_path, &mut secure) {
        eprintln!("[Manager Log] Directory crawler failed: {}", e);
        return Err(e);
    }

    println!("[Manager Log] Transfer completely finished. Exiting.");
    Ok(())
}

fn enabled(selected: &str, name: &str) -> bool {
    selected.eq_ignore_ascii_case("all") || selected.eq_ignore_ascii_case(name)
}

pub fn receive(session: &SessionCode, config: &ReceiveConfig) -> io::Result<()> {
    println!("=== Starting transport receive manager ===");

    let nameplate = session.nameplate;
    let cancel = new_cancel();
    let (tx, rx) = mpsc::channel::<SelectedTransport>();

    let c = cancel.clone();
    let radar_tx = tx.clone();
    thread::spawn(move || {
        if let Ok(s) = p2p_radar::receive(nameplate, c) {
            let _ = radar_tx.send(SelectedTransport::Radar(s));
        }
    });

    let c = cancel.clone();
    let direct_tx = tx.clone();
    thread::spawn(move || {
        if let Ok(s) = p2p_direct::receive(nameplate, c) {
            let _ = direct_tx.send(SelectedTransport::Direct(s));
        }
    });

    let c = cancel.clone();
    let proxy_tx = tx.clone();
    thread::spawn(move || {
        if let Ok(s) = p2p_proxy::connect(nameplate, c) {
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
                "All parallel transport channels failed",
            ));
        }
    };

    cancel.store(true, Ordering::SeqCst);
    drop(rx);

    active_stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    active_stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
    let mut secure = crate::crypto::handshake_responder(active_stream, session.secret.as_bytes())?;
    secure.get_mut().set_read_timeout(None)?;
    secure.get_mut().set_write_timeout(None)?;

    println!("Secure channel established. Starting packet parser...");
    p2p_proxy::receive(secure, mode_label, config)?;

    Ok(())
}
