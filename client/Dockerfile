FROM rust:1.91 as builder

RUN apt-get update && apt-get install -y iptables && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/
COPY . .
RUN cargo build --release

VOLUME ["/usr/local/cargo"]
