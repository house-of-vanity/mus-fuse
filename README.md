# MusFuse

[![Build Status](https://travis-ci.org/joemccann/dillinger.svg?branch=master)](https://travis-ci.org/joemccann/dillinger)

MusFuse is a FUSE filesystem over HTTP for music. It is using [playongo](https://github.com/nixargh/playongo) media library. It's completely written in Rust stable.


# Features
  - Using self hosted media library.
  - Security relies on HTTPS.
  - Any player can be used. (tested on [Cmus](https://github.com/cmus/cmus))
  - Using cache.
  - Leverages Rust correctness.

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

# How to use

```sh
# Compile
$ cargo build --release
# And run
$ ./target/release/musfuse <mountpoint> <server>
```
