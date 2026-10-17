use rand::Rng;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::protocol::ReceiveConfig;

const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SECRET_LEN: usize = 6;

#[derive(Clone, Debug)]
pub struct SessionCode {
    pub nameplate: u32,
    pub secret: String,
}

impl SessionCode {
    pub fn display(&self) -> String {
        format!("{}-{}", self.nameplate, self.secret)
    }
}

pub fn generate_session_code() -> SessionCode {
    let mut rng = rand::thread_rng();
    let nameplate: u32 = rng.gen_range(100000..=999999);
    let secret: String = (0..SECRET_LEN)
        .map(|_| CROCKFORD[rng.gen_range(0..CROCKFORD.len())] as char)
        .collect();

    SessionCode { nameplate, secret }
}

pub fn parse_session_code(input: &str) -> Option<SessionCode> {
    let (plate, secret) = split_code(input.trim())?;

    if plate.len() != 6 || !plate.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let nameplate = plate.parse::<u32>().ok()?;

    if secret.len() != SECRET_LEN {
        return None;
    }

    let mut normalized = String::with_capacity(SECRET_LEN);
    for ch in secret.chars() {
        let upper = ch.to_ascii_uppercase();
        let mapped = match upper {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        if !mapped.is_ascii() || !CROCKFORD.contains(&(mapped as u8)) {
            return None;
        }
        normalized.push(mapped);
    }

    Some(SessionCode {
        nameplate,
        secret: normalized,
    })
}

fn split_code(input: &str) -> Option<(&str, &str)> {
    if let Some((plate, secret)) = input.split_once('-') {
        return Some((plate, secret));
    }

    let mut parts = input.split_whitespace();
    if let (Some(plate), Some(secret), None) = (parts.next(), parts.next(), parts.next()) {
        return Some((plate, secret));
    }

    if input.len() == 12 && input.is_ascii() {
        return Some((&input[..6], &input[6..]));
    }

    None
}

pub fn get_realpath(path_str: &String) -> io::Result<PathBuf> {
    let path = Path::new(path_str);
    fs::canonicalize(path)
}

pub fn configure_receiver(out: Option<&str>, overwrite: bool) -> io::Result<Option<ReceiveConfig>> {
    let chosen = match out {
        Some(dir) => dir.to_string(),
        None => {
            println!("Output directory:");
            let mut line = String::new();
            io::stdin().read_line(&mut line)?;
            line.trim().to_string()
        }
    };

    if chosen.is_empty() {
        return Ok(None);
    }

    let base = PathBuf::from(&chosen);
    fs::create_dir_all(&base)?;
    let base = base.canonicalize()?;

    println!(
        "Files will be saved to {}/. Continue? [Y/n]",
        base.display()
    );
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_ascii_lowercase();
    if answer == "n" || answer == "no" {
        return Ok(None);
    }

    Ok(Some(ReceiveConfig { base, overwrite }))
}

pub fn print_transfer_summary(
    mode_label: &str,
    global_start_time: Instant,
    total_bytes_received: u64,
    total_files_count: u32,
    total_dirs_count: u32,
) {
    let total_elapsed = global_start_time.elapsed().as_secs_f64();

    let average_speed_mb = if total_elapsed > 0.0 {
        (total_bytes_received as f64 / total_elapsed) / (1024.0 * 1024.0)
    } else {
        0.0
    };

    let total_size_mb = total_bytes_received as f64 / (1024.0 * 1024.0);

    println!("\n==================================================");
    println!(
        "             {} TRANSFER SUMMARY                 ",
        mode_label.to_uppercase()
    );
    println!("==================================================");
    println!(" Status:       All data received successfully.");
    println!(" Directories:  {} created", total_dirs_count);
    println!(" Files:        {} received", total_files_count);
    println!(
        " Total Size:   {:.2} MB ({} bytes)",
        total_size_mb, total_bytes_received
    );
    println!(" Time Elapsed: {:.2} seconds", total_elapsed);
    println!(" Average Speed:{:.2} MB/s", average_speed_mb);
    println!("==================================================\n");
}
