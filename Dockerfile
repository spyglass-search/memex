FROM rust:1.68 as builder
RUN apt update -y && apt upgrade -y
# cmake/clang required for llama-rs/whisper-rs builds
RUN apt-get install -y cmake clang

WORKDIR /usr/src
COPY . .
# Required for whisper-rs
RUN rustup component add rustfmt
RUN cargo build -p sightglass --release

FROM debian:stable-slim

WORKDIR /app

RUN apt update \
    && apt install -y openssl ca-certificates curl \
    && apt clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=builder /usr/src/target/release/sightglass ./
ENTRYPOINT [ "./sightglass" ]