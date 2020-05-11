#[macro_use]
extern crate log;
extern crate chrono;

use std::path::Path;
use env_logger::Env;

// Internal staff
mod read_config;
mod entry;

fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
    info!("Logger initialized. Set RUST_LOG=[debug,error,info,warn,trace] Default: info");
    info!("Mus-Fuse {}", env!("CARGO_PKG_VERSION"));

    let config = read_config::read();
    let data = entry::list_dir(config, Path::new("/"));
}
