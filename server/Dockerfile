FROM rust:1.91 as builder
WORKDIR /usr/src/
COPY . .
RUN apt-get update && apt-get install -y lldb clang

VOLUME ["/usr/local/cargo"]