extern crate base64;
extern crate config;
use clap::{App, Arg};
use std::process;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Config {
    pub server: String,
    pub mountpoint: String,
    pub config_file: String,
    pub cache_max_count: u64,
    pub cache_head: u64,
    pub basic_auth: String,
//  pub http_user: String,
//  pub http_pass: String,
}

pub fn read() -> Config {
    // Parse opts and args
    let cli_args = App::new("mus-fuse")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Mount FUSE filesystem with your own remote library.")
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .value_name("ADDRESS")
                .help("Sets a server hosting your library with schema. (https or http)")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mountpoint")
                .short("m")
                .long("mountpoint")
                .value_name("PATH")
                .help("Mount point for library")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("conf")
                .short("c")
                .long("config")
                .value_name("PATH")
                .help("Config file to use")
                .default_value("/etc/mus-fuse.yaml")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cache_max_count")
                .long("cache-max")
                .value_name("COUNT")
                .help("How many files store in cache. [default: 10]")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cache_head")
                .long("cache-head")
                .value_name("KiB")
                .help("How many KiB cache in file beginning for speeding up metadata requests. [default: 768]")
                .required(false)
                .takes_value(true),
        )
        .get_matches();

    // Read config file and env vars
    let conf = cli_args.value_of("conf").unwrap();
    let mut settings = config::Config::default();
    settings = match settings.merge(config::File::with_name(conf)) {
        Ok(conf_content) => {
            info!("Using config file {}", conf);
            conf_content.to_owned()
        }
        Err(e) => {
            warn!("Can't read config file {}", e);
            config::Config::default()
        }
    };
    settings = match settings.merge(config::Environment::with_prefix("MUS")) {
        Ok(conf) => conf.to_owned(),
        Err(_) => config::Config::default(),
    };
    let http_user = match settings.get_str("http_user") {
        Ok(u) => u,
        Err(_) => {
            info!("User for basic auth is not defined.");
            String::new()
        }
    };
    let http_pass = match settings.get_str("http_pass") {
        Ok(u) => u,
        Err(_) => String::new(),
    };
    let server = match settings.get_str("server") {
        Ok(server_cfg) => match cli_args.value_of("server") {
            Some(server_opt) => server_opt.to_string(),
            None => server_cfg,
        },
        Err(_) => match cli_args.value_of("server") {
            Some(server_opt) => server_opt.to_string(),
            None => {
                error!("Server is not set in config nor via run options.");
                process::exit(0x0001)
            }
        },
    };
    let mountpoint = match settings.get_str("mountpoint") {
        Ok(mountpoint_cfg) => match cli_args.value_of("mountpoint") {
            Some(mountpoint_opt) => mountpoint_opt.to_string(),
            None => mountpoint_cfg,
        },
        Err(_) => match cli_args.value_of("mountpoint") {
            Some(mountpoint_opt) => mountpoint_opt.to_string(),
            None => {
                error!("Mount point is not set in config nor via run options.");
                process::exit(0x0001)
            }
        },
    };
    let cache_head = match settings.get_str("cache_head") {
        Ok(cache_head_cfg) => match cli_args.value_of("cache_head") {
            Some(cache_head_opt) => 1024 * cache_head_opt.parse::<u64>().unwrap(),
            None => 1024 * cache_head_cfg.parse::<u64>().unwrap(),
        },
        Err(_) => match cli_args.value_of("cache_head") {
            Some(cache_head_opt) => 1024 * cache_head_opt.parse::<u64>().unwrap(),
            None => 768 * 1024,
        },
    };
    let cache_max_count = match settings.get_str("cache_max_count") {
        Ok(cache_max_count_cfg) => match cli_args.value_of("cache_max_count") {
            Some(cache_max_count_opt) => cache_max_count_opt.parse::<u64>().unwrap(),
            None => cache_max_count_cfg.parse::<u64>().unwrap(),
        },
        Err(_) => match cli_args.value_of("cache_max_count") {
            Some(cache_max_count_opt) => cache_max_count_opt.parse::<u64>().unwrap(),
            None => 10,
        },
    };

    let mut buf = String::new();
    buf.push_str(&http_user);
    buf.push_str(":");
    buf.push_str(&http_pass);

    Config{
        cache_head: cache_head,
        cache_max_count: cache_max_count,
        mountpoint: mountpoint,
        server: server,
        config_file: conf.to_string(),
        basic_auth: base64::encode(buf),
    }
}
