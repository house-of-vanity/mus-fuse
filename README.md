# MusFuse

[![Build Status](https://travis-ci.org/joemccann/dillinger.svg?branch=master)](https://travis-ci.org/joemccann/dillinger)

MusFuse is a FUSE filesystem over HTTP for music. It is using [playongo](https://github.com/nixargh/playongo) media library. It's completely written in Rust stable.


# Features
  - Using self hosted media library.
  - Security relies on HTTPS.
  - Any player can be used. (tested on [Cmus](https://github.com/cmus/cmus))
  - Using cache.
  - Leverages Rust correctness.

# How to use

```sh
# Compile
$ cargo build --release
# And run
$ ./target/release/musfuse <mountpoint> <server>
```