use std::fs;
use std::io;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use crate::protocol;

pub fn send_path(start_path: PathBuf, stream: &mut TcpStream) -> io::Result<()> {
    let metadata = fs::symlink_metadata(&start_path)?;

    if metadata.is_file() {
        let file_name = start_path
            .file_name()
            .map(|os_str| os_str.to_string_lossy().into_owned())
            .unwrap_or_else(|| "shared_file".to_string());

        println!("Sending file: {}", file_name);

        let file_header = protocol::FileHeader {
            size: metadata.len(),
            relative_path: file_name,
            real_path: start_path,
        };
        file_header.send(stream)?;
        file_header.send_content(stream)?;

        stream.write_all(&[protocol::CMD_END_STREAM])?;
        stream.flush()?;
        return Ok(());
    }

    if metadata.is_dir() {
        return crawl_directory(start_path, stream);
    }

    println!("Skipping unsupported path: {}", start_path.display());
    stream.write_all(&[protocol::CMD_END_STREAM])?;
    stream.flush()?;
    Ok(())
}

fn crawl_directory(start_path: PathBuf, stream: &mut TcpStream) -> io::Result<()> {
    let root_name = start_path
        .file_name()
        .map(|os_str| os_str.to_string_lossy().into_owned())
        .unwrap_or_else(|| "shared_folder".to_string());

    println!("Crawler sending root directory layout: {}", root_name);

    let root_header = protocol::DirHeader { relative_path: root_name.clone() };
    root_header.send(stream)?;

    let mut dir_stack = vec![start_path.clone()];

    while let Some(current_dir) = dir_stack.pop() {
        let rel_path =  current_dir
            .strip_prefix(&start_path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        if !rel_path.is_empty() {
            let full_rel_path = format!("{}/{}", root_name, rel_path);
            println!("Crawler sending directory layout: {}", full_rel_path);

            let dir_header = protocol::DirHeader { relative_path: full_rel_path };
            dir_header.send(stream)?;
        }

        let entries = match fs::read_dir(&current_dir) {
            Ok(entries) => entries,
            Err(e) => {
                println!("Skipping unreadable directory: {} ({})", current_dir.display(), e);
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    println!("Skipping unreadable entry in {} ({})", current_dir.display(), e);
                    continue;
                }
            };

            let path = entry.path();

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(e) => {
                    println!("Skipping entry with unknown type: {} ({})", path.display(), e);
                    continue;
                }
            };

            if file_type.is_symlink() {
                println!("Skipping symlink: {}", path.display());
                continue;
            }

            if file_type.is_dir() {
                dir_stack.push(path);
                continue;
            }

            if !file_type.is_file() {
                println!("Skipping special file: {}", path.display());
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(e) => {
                    println!("Skipping unreadable file: {} ({})", path.display(), e);
                    continue;
                }
            };

            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            let file_size = metadata.len();

            let file_rel_path = if rel_path.is_empty() {
                format!("{}/{}", root_name, file_name)
            } else {
                format!("{}/{}/{}", root_name, rel_path, file_name)
            };

            println!(
                "Crawler sending file metadata: {} ({} bytes)",
                file_rel_path, file_size
            );

            let file_header = protocol::FileHeader {
                size: file_size,
                relative_path: file_rel_path,
                real_path: path.clone(),
            };
            file_header.send(stream)?;
            file_header.send_content(stream)?;
            println!("  -> File streaming finished completely.");
        }
    }

    println!("Crawler finished scanning. Sending END_STREAM marker.");
    stream.write_all(&[protocol::CMD_END_STREAM])?;
    stream.flush()?;

    Ok(())
}
