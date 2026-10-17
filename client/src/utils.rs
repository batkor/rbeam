use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::protocol::ReceiveConfig;

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
