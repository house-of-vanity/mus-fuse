extern crate base64;
extern crate clap;
extern crate fuse;
extern crate libc;
extern crate time;
#[macro_use]
extern crate log;
extern crate chrono;
extern crate config;

use clap::{App, Arg};
use env_logger::Env;
use fuse::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::{EIO, ENOENT};
use percent_encoding::percent_decode_str;
use reqwest::{blocking::Client, header::CONTENT_LENGTH};
use serde::Deserialize;
use size_format::SizeFormatterBinary;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    ffi::OsStr,
    fmt,
    path::Path,
    process,
    thread::sleep,
    time::Duration,
};
use time::Timespec;

static mut HTTP_AUTH: String = String::new(); // Basic Auth string.

struct Metrics {
    http_requests: u64,
    connect_errors: u64,
    ingress: u64,
    hit_len_cache: u64,
    hit_data_cache: u64,
    miss_len_cache: u64,
    miss_data_cache: u64,
    server_addr: String,
}

impl fmt::Debug for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
            "http_requests: {}\nconnect_errors: {}\ningress: {}\nhit_len_cache: {}\nhit_data_cache: {}\nmiss_len_cache: {}\nmiss_data_cache: {}\nserver_addr: {}\n", 
            self.http_requests,
            self.connect_errors,
            self.ingress,
            self.hit_len_cache,
            self.hit_data_cache,
            self.miss_len_cache,
            self.miss_data_cache,
            self.server_addr,
        )
    }
}

static mut METRICS: Metrics = Metrics {
    http_requests: 0,
    connect_errors: 0,
    ingress: 0,
    hit_len_cache: 0,
    hit_data_cache: 0,
    miss_len_cache: 0,
    miss_data_cache: 0,
    server_addr: String::new(),
};

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct Track {
    pub id: Option<String>,
    pub name: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub format: Option<String>,
    pub filetype: Option<String>,
    pub path: Option<String>,
    pub size: Option<i64>,
}

fn get_basename(path: Option<&String>) -> Option<String> {
    let base = match percent_decode_str(path.unwrap().as_str()).decode_utf8() {
        Ok(path) => {
            let remote_name = path.into_owned();
            let basename = Path::new(&remote_name).file_name();
            match basename {
                Some(name) => Some(name.to_os_string().into_string().unwrap()),
                None => None,
            }
        }
        Err(_) => None,
    };
    base
}

#[tokio::main]
async fn get_tracks(server: &String) -> Result<Vec<Track>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    unsafe {
        let resp = client
            .get(format!("{}/songs", server).as_str())
            .header("Authorization", format!("Basic {}", HTTP_AUTH))
            .send()
            .await?
            .json::<Vec<Track>>()
            .await?;
        info!("Found {} tracks.", resp.len());
        Ok(resp)
    }
}

#[cfg(target_family = "unix")]
struct JsonFilesystem {
    server: String,
    tree: Vec<Track>,
    attrs: BTreeMap<u64, FileAttr>,
    inodes: BTreeMap<String, u64>,
    buffer_head_index: HashSet<u64>,
    buffer_head_data: HashMap<u64, Vec<u8>>,
    buffer_length: BTreeMap<String, i64>,
    metrics_inode: u64,
    cache_head: u64,
    cache_max_count: u64,
}

#[cfg(target_family = "unix")]
impl JsonFilesystem {
    fn new(
        tree: &Vec<Track>,
        server: String,
        cache_max_count: u64,
        cache_head: u64,
    ) -> JsonFilesystem {
        let mut attrs = BTreeMap::new();
        let mut inodes = BTreeMap::new();
        let ts = time::now().to_timespec();
        let mut total_size: i64 = 0;
        let attr = FileAttr {
            ino: 1,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        };
        attrs.insert(1, attr);
        inodes.insert("/".to_string(), 1);
        for (i, track) in tree.iter().enumerate() {
            let basename = get_basename(track.path.as_ref()).unwrap().to_string();
            debug!(
                "Added inode: {} - {} [{}]",
                i + 2,
                basename,
                track.size.unwrap()
            );
            total_size = total_size + track.size.unwrap();
            let attr = FileAttr {
                ino: i as u64 + 2,
                size: track.size.unwrap() as u64,
                blocks: 0,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: FileType::RegularFile,
                perm: 0o644,
                nlink: 0,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            };
            attrs.insert(attr.ino, attr);
            inodes.insert(basename.clone(), attr.ino);
        }
        // Metrics file
        let metrics_inode = 2 + tree.len() as u64;
        let metrics_attr = FileAttr {
            ino: metrics_inode,
            size: 4096,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        };
        attrs.insert(metrics_attr.ino, metrics_attr);
        inodes.insert("METRICS.TXT".to_string(), metrics_attr.ino);
        info!(
            "Filesystem initialized. Size: {} files, {}B in total.",
            inodes.len(),
            (SizeFormatterBinary::new(total_size as u64))
        );
        JsonFilesystem {
            server: server,
            tree: tree.clone(),
            attrs: attrs,
            inodes: inodes,
            buffer_head_data: HashMap::new(),
            buffer_head_index: HashSet::new(),
            buffer_length: BTreeMap::new(),
            metrics_inode: metrics_inode,
            cache_head: cache_head,
            cache_max_count: cache_max_count,
        }
    }
}

#[cfg(target_family = "unix")]
impl Filesystem for JsonFilesystem {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr(ino={})", ino);
        match self.attrs.get(&ino) {
            Some(attr) => {
                let ttl = Timespec::new(1, 0);
                reply.attr(&ttl, attr);
            }
            None => reply.error(ENOENT),
        };
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup(parent={}, name={})", parent, name.to_str().unwrap());
        let inode = match self.inodes.get(name.to_str().unwrap()) {
            Some(inode) => inode,
            None => {
                reply.error(ENOENT);
                return;
            }
        };
        match self.attrs.get(inode) {
            Some(attr) => {
                let ttl = Timespec::new(1, 0);
                debug!("{:#?}", attr);
                reply.entry(&ttl, attr, 0);
            }
            None => reply.error(ENOENT),
        };
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        reply: ReplyData,
    ) {
        // return usage statistics
        if ino == self.metrics_inode {
            unsafe {
                let metrics_str = format!("{:#?}", METRICS);
                reply.data(&metrics_str.as_bytes());
            }
            return;
        }

        // cleaning cache
        if self.buffer_head_index.len() > self.cache_max_count as usize {
            let mut iter = self.buffer_head_index.iter().filter(|&x| *x != ino);
            let old_entry = iter.next().unwrap();
            self.buffer_head_data.remove(old_entry);
            let old_entry_copy = old_entry.clone();
            self.buffer_head_index.remove(&old_entry_copy);
            let basename = &self.tree[(ino - 2) as usize].path.as_ref();
            debug!(
                "{} - Cache dropped for: {} ",
                ino,
                get_basename(*basename).unwrap().to_string()
            );
        }
        debug!(
            "{} - read(ino={}, fh={}, offset={}, size={}) ",
            ino, ino, fh, offset, size
        );

        let url = &self.tree[(ino - 2) as usize].path.as_ref().unwrap();
        let id = &self.tree[(ino - 2) as usize].id.as_ref().unwrap();
        let full_url = format!("{}{}", self.server, url);
        let chunk: Vec<u8>;
        let content_length: i64;
        let client = Client::new();

        // content_length cache.
        if self.buffer_length.contains_key(id.as_str()) {
            content_length = self.buffer_length[id.as_str()];
            debug!("{} - Hit length cache", ino);
            unsafe {
                METRICS.hit_len_cache += 1;
            }
        } else {
            unsafe {
                content_length = match client
                    .head(full_url.as_str())
                    .header("Authorization", format!("Basic {}", HTTP_AUTH))
                    .send()
                {
                    Ok(content) => {
                        let content_length = match content.headers().get(CONTENT_LENGTH) {
                            Some(header_content) => {
                                header_content.to_str().unwrap().parse::<i64>().unwrap()
                            }
                            None => {
                                reply.error(EIO);
                                return;
                            }
                        };
                        content_length
                    }
                    Err(err) => {
                        let name = &self.tree[(ino - 2) as usize].path.as_ref();
                        let basename = get_basename(*name).unwrap().to_string();
                        error!("An error fetching file {}. {}", basename, err);
                        METRICS.connect_errors += 1;
                        reply.error(EIO);
                        return;
                    }
                };
            }
            unsafe {
                METRICS.http_requests += 1;
            }
            self.buffer_length.insert(id.to_string(), content_length);
            debug!("{} - Miss length cache", ino);
            unsafe {
                METRICS.miss_len_cache += 1;
            }
        }
        // Check for API wrong file size here
        if content_length > offset {
            debug!("{} - Content len {:?} ", ino, content_length);
            let end_of_chunk = if size - 1 + offset as u32 > content_length as u32 {
                content_length
            } else {
                (size + offset as u32) as i64
            };
            let range = format!("bytes={}-{}", offset, end_of_chunk - 1);

            // if it's beginning of file...
            if end_of_chunk < self.cache_head as i64 {
                // looking for CACHE_HEAD bytes file beginning in cache
                if self.buffer_head_data.contains_key(&ino) {
                    // Cache found
                    debug!("{} - Hit data cache", ino);
                    unsafe {
                        METRICS.hit_data_cache += 1;
                    }
                    chunk = self.buffer_head_data[&ino][offset as usize..end_of_chunk as usize]
                        .to_vec()
                        .clone();
                    reply.data(&chunk);
                } else {
                    // Cache doesn't found
                    debug!("{} - Miss data cache", ino);
                    unsafe {
                        METRICS.miss_data_cache += 1;
                    }
                    // Fetch file head (CACHE_HEAD)
                    let response: Vec<u8>;
                    unsafe {
                        response = match client
                            .get(full_url.as_str())
                            .header(
                                "Range",
                                format!(
                                    "bytes=0-{}",
                                    if self.cache_head as i64 > content_length {
                                        content_length - 1
                                    } else {
                                        self.cache_head as i64 - 1
                                    }
                                ),
                            )
                            .header("Authorization", format!("Basic {}", HTTP_AUTH))
                            .send()
                        {
                            Ok(content) => content.bytes().unwrap().to_vec(),
                            Err(err) => {
                                let name = &self.tree[(ino - 2) as usize].path.as_ref();
                                let basename = get_basename(*name).unwrap().to_string();
                                error!("An error fetching file {}. {}", basename, err);
                                METRICS.connect_errors += 1;
                                reply.error(EIO);
                                return;
                            }
                        };
                    }
                    unsafe {
                        METRICS.http_requests += 1;
                        METRICS.ingress += response.len() as u64;
                    }
                    // Save cache
                    self.buffer_head_data.insert(ino, response.to_vec());
                    self.buffer_head_index.insert(ino);
                    chunk = response[offset as usize..end_of_chunk as usize].to_vec();
                    reply.data(&chunk);
                }
                debug!("{} - Chunk len: {:?} ", ino, chunk.len());
                return;
            }
            // If it isn't a beginning of file don't cache it and fetch over HTTP directly.
            let response: Vec<u8>;
            unsafe {
                response = match client
                    .get(full_url.as_str())
                    .header("Range", &range)
                    .header("Authorization", format!("Basic {}", HTTP_AUTH))
                    .send()
                {
                    Ok(content) => content.bytes().unwrap().to_vec(),
                    Err(err) => {
                        let name = &self.tree[(ino - 2) as usize].path.as_ref();
                        let basename = get_basename(*name).unwrap().to_string();
                        error!("An error fetching file {}. {}", basename, err);
                        METRICS.connect_errors += 1;
                        reply.error(EIO);
                        return;
                    }
                };
            }
            unsafe {
                METRICS.http_requests += 1;
                METRICS.ingress += response.len() as u64;
            }
            chunk = response.to_vec().clone();
            reply.data(&chunk);
            debug!(
                "{} - Len: {}, Chunk {} - {}",
                ino,
                chunk.len(),
                offset,
                offset + chunk.len() as i64
            );
        } else {
            // Wrong filesize detected.
            warn!(
                "{} - Wrong offset. Len is {} but offset {}",
                ino, content_length, offset
            );
            reply.data(&[]);
        }
        return;
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        debug!("readdir(ino={}, fh={}, offset={})", ino, fh, offset);
        if ino == 1 {
            if offset == 0 {
                reply.add(1, 0, FileType::Directory, ".");
                reply.add(1, 1, FileType::Directory, "..");
            }
            for (i, (key, &inode)) in self.inodes.iter().enumerate().skip(offset as usize) {
                if inode == 1 {
                    continue;
                }
                reply.add(inode, (i + 1) as i64, FileType::RegularFile, key);
            }
            reply.ok();
        } else {
            reply.error(ENOENT);
        }
    }
}

fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
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

    info!("Logger initialized. Set RUST_LOG=[debug,error,info,warn,trace] Default: info");
    info!("Mus-Fuse {}", env!("CARGO_PKG_VERSION"));

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

    unsafe {
        METRICS.server_addr = server.clone();
        let mut buf = String::new();
        buf.push_str(&http_user);
        buf.push_str(":");
        buf.push_str(&http_pass);
        HTTP_AUTH = base64::encode(buf)
    }
    let lib = match get_tracks(&server) {
        Ok(library) => library,
        Err(err) => {
            error!("Can't fetch library from remote server. Probably server is not running or auth failed. {}", err);
            error!(
                "Provide Basic Auth credentials by setting envs MUS_HTTP_USER and MUS_HTTP_PASS or providing config.",
            );
            process::exit(0x0001)
        }
    };
    info!("Remote library host: {}", &server);
    let fs = JsonFilesystem::new(&lib, server, cache_max_count, cache_head);
    let options = [
        "-o",
        "ro",
        "-o",
        "fsname=musfs",
        "-o",
        "sync_read",
        "-o",
        "auto_unmount",
        "-o",
        "allow_other",
    ]
    .iter()
    .map(|o| o.as_ref())
    .collect::<Vec<&OsStr>>();

    info!(
        "Caching {}B in head of files.",
        SizeFormatterBinary::new(cache_head as u64)
    );
    info!("Max cache is {} files.", cache_max_count);
    info!("Fuse mount options: {:?}", options);
    let _mount: fuse::BackgroundSession;
    unsafe {
        _mount = fuse::spawn_mount(fs, &mountpoint, &options).expect("Couldn't mount filesystem");
    }
    ctrlc::set_handler(move || {
        println!("Exitting...");
        process::exit(0x0000);
    })
    .expect("Error setting Ctrl-C handler");
    loop {
        sleep(Duration::from_millis(300));
    }
}
