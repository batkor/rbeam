# rbeam

[English](README.md) | [Русский](README.ru.md)

A peer-to-peer file and directory transfer tool with a self-hostable signaling and relay server.
Traffic is end-to-end encrypted with a key derived from a short sharing code.

Binaries: `rbeam` (client) and `rbeam-relay` (server).

## What it does

Transfer files and directories between two clients using a short numeric code. The server
only helps peers find each other and relays traffic when a direct connection is not possible;
it never stores your data.

## Transports

The sender starts three transports in parallel. The first one that establishes a real
connection wins and the rest are cancelled:

- **RADAR** — LAN discovery over UDP broadcast, then a direct TCP connection.
- **DIRECT** — the server gives the receiver the sender's address, then the clients connect
  directly.
- **PROXY** — traffic is relayed through the server when a direct path is unavailable.

## Advantages

- **LAN-first** — transfers stay on the local network whenever possible.
- **Automatic fallback** — if direct connectivity fails, the relay keeps it working.
- **No data storage on the server** — it is a signaler/relay, not a vault.
- **End-to-end encrypted** — SPAKE2 key agreement from the code, then ChaCha20-Poly1305;
  the server only ever sees ciphertext.
- **Self-hostable** — build your own instance with your own host and port baked in.
- **Safe receive** — files are written only into a directory you choose, with path validation.

## Self-host configuration

Addresses are compiled in at build time through `.cargo/config.toml`:

```toml
[env]
SERVER_BIND = "0.0.0.0:80"   # local address the server listens on
SERVER_ADDR = "host:80"       # public address clients connect to
```

Copy `.cargo/config.toml.example` to `.cargo/config.toml` (gitignored) and edit it.

```sh
cargo build --release

./target/release/rbeam-relay                 # start your server
./target/release/rbeam --path /path/to/send  # sender (prints a code)
./target/release/rbeam                       # receiver (enter the code)
```

The receiving client asks where to save the data; there is no default directory.

## Usage

Sender:

```sh
cargo run -- --path /tmp/fixture
```

It prints a sharing code like `123456-ABCDEF` (a public nameplate plus a secret).

Receiver:

```sh
cargo run
# enter the code, then confirm/choose the output directory
```

Options:

- `--out <dir>` — output directory (otherwise asked interactively).
- `--override` — replace existing files (default: skip and log).
- `--transport <all|radar|direct|proxy>` — force a single sending transport (default: all).

The receiver always tries all three transports; the sender can be limited with `--transport`.

## Docker (development)

```sh
docker compose build
docker compose up -d

docker compose exec client1 bash    # cargo run -- --path /tmp/fixture
docker compose exec client2 bash    # cargo run
```

The images do not pre-build the binary; compilation happens on `cargo run` inside the
container. `.cargo/config.toml` is mounted into each container, so `env!`-based addresses
are available.

## Security

- The code has two parts: a public **nameplate** (used by the server to find the session)
  and a **secret** (never sent to the server). The secret is used in SPAKE2 to derive a
  strong session key; after that, data is encrypted and authenticated with ChaCha20-Poly1305.
- A wrong secret fails key confirmation and the connection is dropped.
- The receiver treats the sender as untrusted: paths are validated and confined to `--out`;
  absolute paths, `..`, drive letters and NUL are rejected.
- Existing files are skipped unless `--override`.
- Symlinks and special files are skipped on the sender.
- The server relays only ciphertext. The short secret limits online guessing, so pick a
  network and a code you trust.
