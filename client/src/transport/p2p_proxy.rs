use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::protocol;
use crate::transport::{Cancel, is_cancelled, read_line};

pub fn send(code: u32, cancel: Cancel) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect("server:80")?;

    let registration_msg = format!("PROXY:{}\n", code);
    stream.write_all(registration_msg.as_bytes())?;
    stream.flush()?;

    let server_response = read_line(&mut stream, &cancel)?;
    if server_response.starts_with("ERR") {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Code conflict on server"));
    }

    println!("Proxy bridge socket registered successfully on server. Awaiting peer...");

    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    let mut wake = [0u8; 1];

    loop {
        if is_cancelled(&cancel) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }

        match stream.read(&mut wake) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Proxy bridge closed by server",
                ));
            }
            Ok(_) => {
                stream.set_read_timeout(None)?;
                println!("Peer connected through the proxy bridge.");
                return Ok(stream);
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
}

pub fn connect(code: u32, cancel: Cancel) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect("server:80")?;

    let request = format!("GET:PROXY:{}\n", code);
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let response = read_line(&mut stream, &cancel)?;
    if !response.starts_with("OK") {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No proxy sender registered"));
    }

    Ok(stream)
}

pub fn receive(mut stream: TcpStream, mode_label: &str) -> io::Result<()> {
    let global_start_time = std::time::Instant::now();
    let mut total_bytes_received: u64 = 0;
    let mut total_files_count: u32 = 0;
    let mut total_dirs_count: u32 = 0;

    loop {
        let mut command_marker = [0; 1];

        if stream.read_exact(&mut command_marker).is_err() {
            println!("[Parser Log] Connection closed abruptly by remote side.");
            break;
        }

        match command_marker[0] {
            protocol::CMD_CREATE_DIR => {
                println!("[Parser Log] Caught marker -> Command: CREATE_DIR");
                let dir = protocol::DirHeader::receive(&mut stream)?;
                println!(
                    "[Parser Log]   -> Success: Directory structure created on disk: \"{}\"",
                    dir.relative_path
                );
                total_dirs_count += 1;
            },

            protocol::CMD_START_FILE => {
                println!("[Parser Log] Caught marker -> Command: START_FILE");
                let file_meta = protocol::FileHeader::receive(&mut stream)?;
                println!(
                    "[Parser Log]   -> Receiving file: \"{}\" ({} bytes)",
                    file_meta.relative_path, file_meta.size
                );
                file_meta.receive_content(&mut stream)?;
                println!("[Parser Log]   -> Success: File saved completely.");
                total_bytes_received += file_meta.size;
                total_files_count += 1;
            },

            protocol::CMD_END_STREAM => {
                println!("[Parser Log] Caught marker -> Command: END_STREAM");
                println!("[Parser Log] All directory layouts received successfully. Exiting cleanly.");

                crate::utils::print_transfer_summary(
                    mode_label,
                    global_start_time,
                    total_bytes_received,
                    total_files_count,
                    total_dirs_count
                );
                break;
            },

            _ => {
                println!("[Parser Log] Warning: Unknown protocol marker received: {}", command_marker[0]);
                break;
            }
        }
    }
    Ok(())
}
