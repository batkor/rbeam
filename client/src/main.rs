use clap::{Parser, ValueEnum};
use std::io;

pub mod protocol;
pub mod path_crawler;
pub mod utils;
pub mod transport;

#[derive(Clone, Debug, ValueEnum)]
enum TransportKind {
    All,
    Radar,
    Direct,
    Proxy,
}

impl TransportKind {
    fn as_str(&self) -> &'static str {
        match self {
            TransportKind::All => "all",
            TransportKind::Radar => "radar",
            TransportKind::Direct => "direct",
            TransportKind::Proxy => "proxy",
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The path to stream.
    #[arg(short, long)]
    path: Option<String>,

    /// Force a single sending transport for debugging.
    /// Default: all (parallel race).
    #[arg(short, long, value_enum, default_value = "all")]
    transport: TransportKind,

    /// Directory to write received files into. If omitted, the client asks.
    #[arg(short, long)]
    out: Option<String>,

    /// Override existing files instead of skipping them.
    #[arg(long = "override")]
    overwrite: bool,
}

fn main() {
    let args = Args::parse();

    match args.path {
        // Sender mode.
        Some(path) => {
            if let Err(e) = transport::manager::send(&path, args.transport.as_str()) {
                eprintln!("Sender error: {}", e);
            }
        }
        // Getter mode.
        None => {
            println!("Input code:");
            let mut code_str = String::new();

            if let Err(e) = io::stdin().read_line(&mut code_str) {
                eprintln!("Receiver error (failed to read stdin): {}", e);
                return;
            }

            let cleaned_code = match code_str.trim().parse::<u32>() {
                Ok(num) => num,
                Err(_) => {
                    eprintln!("Receiver error: Invalid code format. Must be a number.");
                    return;
                }
            };

            match crate::utils::configure_receiver(args.out.as_deref(), args.overwrite) {
                Ok(Some(config)) => {
                    if let Err(e) = transport::manager::receive(cleaned_code, &config) {
                        eprintln!("Receiver error: {}", e);
                    }
                }
                Ok(None) => {
                    println!("Cancelled: no output directory.");
                }
                Err(e) => {
                    eprintln!("Receiver error (failed to configure output): {}", e);
                }
            }
        }
    }
}
