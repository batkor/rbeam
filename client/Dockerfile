FROM rust:1.91 as builder

RUN rustup component add rustfmt clippy
RUN apt-get update && apt-get install -y iptables && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/
COPY . .

VOLUME ["/usr/local/cargo"]
