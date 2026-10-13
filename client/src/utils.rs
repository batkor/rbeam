use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn get_realpath(path_str: &String) -> io::Result<PathBuf> {
    let path = Path::new(path_str);
    fs::canonicalize(path)
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
    println!("             {} TRANSFER SUMMARY                 ", mode_label.to_uppercase());
    println!("==================================================");
    println!(" Status:       All data received successfully.");
    println!(" Directories:  {} created", total_dirs_count);
    println!(" Files:        {} received", total_files_count);
    println!(" Total Size:   {:.2} MB ({} bytes)", total_size_mb, total_bytes_received);
    println!(" Time Elapsed: {:.2} seconds", total_elapsed);
    println!(" Average Speed:{:.2} MB/s", average_speed_mb);
    println!("==================================================\n");
}
