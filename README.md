# MusFuse

[![Build Status](https://github.com/house-of-vanity/mus_fuse/workflows/Build%20and%20publish/badge.svg)](https://github.com/house-of-vanity/mus_fuse/actions)

MusFuse is a FUSE filesystem over HTTP for music. It is using [playongo](https://github.com/nixargh/playongo) media library. It's completely written in Rust stable.


# Features
  - Using self hosted media library.
  - Security relies on HTTPS.
  - Any player can be used. (tested on [Cmus](https://github.com/cmus/cmus))
  - Using cache.
  - Leverages Rust correctness.
  
## How to use
Here is a [binary release](https://github.com/house-of-vanity/mus_fuse/releases/latest) or compile it yourself. Anyway mond about dependencies listed below.

```sh
# Compile
$ cargo build --release

# And run
# to baypass Basic Auth set 
# $HTTP_USER and $HTTP_PASS 
# environment variables before run.
$ ./target/release/musfuse <mountpoint> <server>

# To get metrics
$ cat <mountpoint>/METRICS.TXT
http_requests: 1818
ingress: 243595644
hit_len_cache: 1878
hit_data_cache: 82
miss_len_cache: 11
miss_data_cache: 11
server_addr: https://mus.hexor.ru

```

## Dependencies

FUSE must be installed to build or run programs that use fuse-rs (i.e. kernel driver and libraries. Some platforms may also require userland utils like `fusermount`). A default installation of FUSE is usually sufficient.

To build fuse-rs or any program that depends on it, `pkg-config` needs to be installed as well.

### Linux

[FUSE for Linux][libfuse] is available in most Linux distributions and usually called `fuse`. 

Install on Arch Linux:

```sh
sudo pacman -S fuse
```

Install on Debian based system:

```sh
sudo apt-get install fuse
```

Install on CentOS:

```sh
sudo yum install fuse
```

To build, FUSE libraries and headers are required. The package is usually called `libfuse-dev` or `fuse-devel`. Also `pkg-config` is required for locating libraries and headers.

```sh
sudo apt-get install libfuse-dev pkg-config
```

```sh
sudo yum install fuse-devel pkgconfig
```


