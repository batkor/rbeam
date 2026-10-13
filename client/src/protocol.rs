use std::fs::File;
use std::{fs, io};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

pub const CMD_CREATE_DIR: u8 = 1;
pub const CMD_START_FILE: u8 = 2;
pub const CMD_END_STREAM: u8 = 3;

/// Normalizes a wire path into a safe relative one: both separators are
/// handled, `..` is neutralized, and `:`/NUL are rejected.
pub fn sanitize_relative_path(raw: &str) -> Option<PathBuf> {
    let mut safe = PathBuf::new();

    for part in raw.split(|c: char| c == '/' || c == '\\') {
        match part {
            "" | "." => continue,
            ".." => {
                safe.pop();
            }
            _ if part.contains(':') || part.contains('\0') => return None,
            _ => safe.push(part),
        }
    }

    if safe.as_os_str().is_empty() {
        return None;
    }

    Some(safe)
}

pub struct DirHeader {
    pub relative_path: String,
}

impl DirHeader {

    pub fn send(&self, stream: &mut TcpStream) -> io::Result<()> {
        stream.write_all(&[CMD_CREATE_DIR])?;

        let path_bytes = self.relative_path.as_bytes();
        let path_len = path_bytes.len() as u16;

        stream.write_all(&path_len.to_be_bytes())?;
        stream.write_all(path_bytes)?;
        stream.flush()
    }

    pub fn receive(stream: &mut TcpStream) -> io::Result<Self> {
        let mut len_buffer = [0; 2];
        stream.read_exact(&mut len_buffer)?;
        let path_len = u16::from_be_bytes(len_buffer) as usize;

        let mut path_buffer = vec![0; path_len];
        stream.read_exact(&mut path_buffer)?;
        let raw_path = String::from_utf8_lossy(&path_buffer);
        let relative_path = sanitize_relative_path(&raw_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unsafe path rejected"))?;
        fs::create_dir_all(&relative_path)?;

        Ok(DirHeader {
            relative_path: relative_path.to_string_lossy().into_owned(),
        })
    }
}

pub struct FileHeader {
    pub size: u64,
    pub relative_path: String,
    pub real_path: PathBuf,
}

impl FileHeader {

    pub fn send(&self, stream: &mut TcpStream) -> io::Result<()> {
        stream.write_all(&[CMD_START_FILE])?;
        stream.write_all(&self.size.to_be_bytes())?;

        let path_bytes = self.relative_path.as_bytes();
        let path_len = path_bytes.len() as u16;

        stream.write_all(&path_len.to_be_bytes())?;
        stream.write_all(path_bytes)?;
        stream.flush()
    }

    pub fn send_content(&self, stream: &mut TcpStream) -> io::Result<()> {
        let mut file = File::open(&self.real_path)?;
        let mut file_buf = [0; 8192];
        let mut bytes_sent = 0;

        while bytes_sent < self.size {
            let n = file.read(&mut file_buf)?;
            if n == 0 { break; }

            stream.write_all(&file_buf[..n])?;
            bytes_sent += n as u64;
        }
        stream.flush()
    }

    pub fn receive(stream: &mut TcpStream) -> io::Result<Self> {
        let mut size_buffer = [0; 8];
        stream.read_exact(&mut size_buffer)?;
        let size = u64::from_be_bytes(size_buffer);

        let mut len_buffer = [0; 2];
        stream.read_exact(&mut len_buffer)?;
        let path_len = u16::from_be_bytes(len_buffer) as usize;

        let mut path_buffer = vec![0; path_len];
        stream.read_exact(&mut path_buffer)?;
        let raw_path = String::from_utf8_lossy(&path_buffer);
        let relative_path = sanitize_relative_path(&raw_path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unsafe path rejected"))?;

        Ok(FileHeader {
            size,
            relative_path: relative_path.to_string_lossy().into_owned(),
            real_path: PathBuf::new()
        })
    }

    pub fn receive_content(&self, stream: &mut TcpStream) -> io::Result<()> {
        let mut new_file = std::fs::File::create(&self.relative_path)?;
        let mut bytes_received = 0;
        let mut file_buf = [0; 8192];

        let start_time = std::time::Instant::now();

        while bytes_received < self.size {
            let to_read = std::cmp::min(file_buf.len() as u64, self.size - bytes_received) as usize;

            stream.read_exact(&mut file_buf[..to_read])?;
            new_file.write_all(&file_buf[..to_read])?;

            bytes_received += to_read as u64;

            let percent = if self.size > 0 {
                (bytes_received * 100) / self.size
            }
            else {
                100
            };

            let elapsed = start_time.elapsed().as_secs_f64();
            let speed_mb = if elapsed > 0.0 {
                (bytes_received as f64 / elapsed) / (1024.0 * 1024.0)
            } else {
                0.0
            };

            print!(
                "\r[Parser Log]   -> Progress: {}% | Received: {}/{} bytes | Speed: {:.2} MB/s",
                percent, bytes_received, self.size, speed_mb
            );
            let _ = std::io::stdout().flush();
        }

        new_file.flush()?;
        println!();
        Ok(())
    }
}
