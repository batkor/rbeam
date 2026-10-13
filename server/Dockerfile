FROM rust:1.91 as builder
WORKDIR /usr/src/
COPY . .
RUN apt-get update && apt-get install -y lldb clang
RUN cargo build --release

VOLUME ["/usr/local/cargo"]