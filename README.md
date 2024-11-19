# OG Loc

Open Graph image generator for crates.io

## Building

1. Install Rust following the instructions on <https://rustup.rs>.
2. Install the Fira Sans font on your machine: <https://fonts.google.com/specimen/Fira+Sans> (a later version might include the fonts in the binray itself)
3. Run `cargo build` for a debug build, or `cargo build --release` for an optimized build

## Running
OG Loc gets its data from Crates.io database dumps, which are loaded in a set of hash maps each time the application starts.
First, you'll need the latest Crates.io database dump, which you can fetch from <https://static.crates.io/db-dump.tar.gz
>. Using `wget`:

```bash
wget https://static.crates.io/db-dump.tar.gz
```

OG Loc can either perform a a one-shot image generation, do a bulk job or run as a HTTP server.

```bash
$ cargo run -q -- --help
Usage: og-loc [OPTIONS] <COMMAND>

Commands:
  serve     Run the server
  one-shot  Do a single conversion
  bulk      Do a bulk conversion
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --db-dump-path <DB_DUMP_PATH>  The path of the database dump [env: DB_DUMP_PATH=] [default: ./db-dump.tar.gz]
  -h, --help                         Print help
```

### One shot
To run generate a single Open Graph image for a crate, use the `one-shot` subcommand:

```bash
$ cargo run -q -- one-shot --help
Do a single conversion

Usage: og-loc one-shot --name <NAME> --out <OUT_PATH>

Options:
  -n, --name <NAME>     The name of the crate [env: NAME=]
  -o, --out <OUT_PATH>  The path to the PNG output file [env: OUT_PATH=]
  -h, --help            Print help
```

For instance, to get an image for the `knien` crate at version `0.0.8`, run

```bash
# Unoptimized build
cargo run -- one-shot --name knien --version 0.0.8 --out knien-og.png
# Optimized build, significantly improves performance
cargo run --release -- one-shot --name knien --version 0.0.8 --out knien-og.png
```

Result:

![image](./src/snapshots/og_loc__convert__tests__render_png.snap.png)

*note that this is a very simple prototype and not much work has been put in the styling of the images,
but this image could fairly easily be styled to match the crates.io style.
Futhermore, adding more information to the image is quite trivial.*

### Bulk job
To run a bulk job for a number of crates, use the `bulk` subcommand:

```bash
$ cargo run -q -- bulk --help
Do a bulk conversion

Usage: og-loc bulk [OPTIONS] --in <INPUT> --out <OUT_FOLDER>

Options:
  -f, --force             Force overwrite the output [env: FORCE=]
  -i, --in <INPUT>        Input specifier. Either a comma-separated list of crate names, a path to a file containing a newline-separated list of crate names, or `-`, indicating stdin. Will first attempt to match input with `-`, then parse it as a comma-separated list of crate names, and then fall back to a path, only failing if an empty value is passed [env: INPUT=]
  -o, --out <OUT_FOLDER>  The path of the folder to which the PNGs should be written [env: OUT_FOLDER=]
  -h, --help              Print help
```

For instance, to generate a number of images for line break separated crate names specified in `test.txt`,
and write them to the `out` folder, run

```bash
# Unoptimized build
cargo run -- bulk --in test.txt -o test
# Optimized build, significantly improves performance
cargo run --release -- bulk -in test.txt -out test
```

### Server
To run the OG Loc server, use the `serve` subcommand:

```bash
$ cargo run -q -- serve --help
Run the server

Usage: og-loc serve [OPTIONS]

Options:
  -a, --addr <ADDR>  The socket address to listen on [env: ADDR=] [default: 127.0.0.1:3000]
  -h, --help         Print help
```

For instance, to serve locally from port 3000, run

```bash
# Unoptimized build
cargo run -- serve --addr 0.0.0.0:3000
# Optimized build, significantly improves performance
cargo run --reelase -- serve --addr 0.0.0.0:3000
```

Open your browser, and navigate to `http://localhost:3000/og/<CRATE_NAME>`
For instance, to get an image for the `knien` crate, navigate to <http://localhost:3000/og/knien>

## Internals
OG Loc uses the awesome [Typst](https://typst.app/) typesetting system internally to render the PNG images from a
[Jinja2 template](./template.typ.j2) that gets filled with information from crates.io.
The HTTP server is implented using Axum, and the Crates.io database is loaded using [`db_dump`](https://github.com/dtolnay/db-dump).

