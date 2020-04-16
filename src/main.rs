// Fuse staff
extern crate fuse;
extern crate libc;
extern crate time;
use fuse::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::ENOENT;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_LENGTH;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use time::Timespec;
//use http::Method;

// Download lib staff
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
}

const API_URL: &str = "https://mus.hexor.ru";

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
async fn get_tracks() -> Result<Vec<Track>, Box<dyn std::error::Error>> {
    let resp = reqwest::get(format!("{}/songs", API_URL).as_str())
        .await?
        .json::<Vec<Track>>()
        .await?;
    println!("Found {} tracks.", resp.len());
    Ok(resp)
}

#[cfg(target_family = "unix")]
struct JsonFilesystem {
    tree: Vec<Track>,
    attrs: BTreeMap<u64, FileAttr>,
    inodes: BTreeMap<String, u64>,
    buffer_data: Vec<u8>,
    buffer_name: String,
    buffer_length: HashMap<String, i64>,
}

#[cfg(target_family = "unix")]
impl JsonFilesystem {
    fn new(tree: &Vec<Track>) -> JsonFilesystem {
        let mut attrs = BTreeMap::new();
        let mut inodes = BTreeMap::new();
        let ts = time::now().to_timespec();
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
            let attr = FileAttr {
                ino: i as u64 + 2,
                size: 1024 * 1024 * 1024 as u64,
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
        JsonFilesystem {
            tree: tree.clone(),
            attrs: attrs,
            inodes: inodes,
            buffer_data: Vec::new(),
            buffer_name: "".to_string(),
            buffer_length: HashMap::new(),
        }
    }
}

#[cfg(target_family = "unix")]
impl Filesystem for JsonFilesystem {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        //println!("getattr(ino={})", ino);
        match self.attrs.get(&ino) {
            Some(attr) => {
                let ttl = Timespec::new(1, 0);
                reply.attr(&ttl, attr);
            }
            None => reply.error(ENOENT),
        };
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        //println!("lookup(parent={}, name={})", parent, name.to_str().unwrap());
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
        println!(
            "read(ino={}, fh={}, offset={}, size={})",
            ino, fh, offset, size
        );
        //let mus = fs::read("/home/ab/Downloads/Mizuki.mp3").unwrap();
        let url = &self.tree[(ino - 2) as usize].path.as_ref().unwrap();
        let full_url = format!("{}/{}", API_URL, url);
        let mut full_track: Vec<u8> = Vec::new();
        //if self.buffer_length[full_url] == full_url {
        //full_track = self.buffer_data.clone();
        //println!("Hit cache!");
        //} else {
        let client = Client::new();
        //let req_builder = client.request(Method::GET, full_url.as_str());
        let mut resp = client.head(full_url.as_str()).send().unwrap();
        let content_length = resp
            .headers()
            .get(CONTENT_LENGTH)
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        println!("Len {:?}", content_length);
        let range = format!("bytes={}-{}", offset, offset - 1 + size as i64);
        println!("Range: {:?}", range);
        resp = client
            .get(full_url.as_str())
            .header("Range", &range)
            .send()
            .unwrap();
        let test = resp.bytes().unwrap();
        full_track = test.to_vec().clone();
        //self.buffer_data = full_track.clone();
        //self.buffer_name = full_url;
        //println!("Miss cache!");
        //}
        /*
        let mut chunk_end = size as usize + offset as usize;
        if chunk_end >= content_length {
            chunk_end = content_length;
        }
        if offset as usize >= content_length {
            reply.data(&full_track[(content_length - 1) as usize..chunk_end as usize]);
        } else {
            reply.data(&full_track[offset as usize..chunk_end as usize]);
        }*/
        reply.data(&full_track);
        println!(
            "Len: {}, chunk {} - {}",
            full_track.len(),
            offset,
            offset + size as i64
        );
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
        //println!("readdir(ino={}, fh={}, offset={})", ino, fh, offset);
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
    let lib = get_tracks().unwrap();
    let fs = JsonFilesystem::new(&lib);
    let mountpoint = match env::args().nth(1) {
        Some(path) => path,
        None => {
            println!("Usage: {} <MOUNTPOINT>", env::args().nth(0).unwrap());
            return;
        }
    };
    fuse::mount(fs, &mountpoint, &[]).expect("Couldn't mount filesystem");
}
