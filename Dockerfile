FROM rust:1.68 as builder

ARG GIT_HASH=unknown
ENV GIT_HASH=$GIT_HASH

RUN apt update -y && apt upgrade -y
# cmake/clang required for llama-rs/whisper-rs builds
RUN apt-get install -y cmake clang

WORKDIR /usr/src
COPY . .
# Required for whisper-rs
RUN rustup component add rustfmt
RUN cp .env.template .env
RUN cargo build -p memex --release

FROM debian:stable-slim

WORKDIR /app

RUN apt update \
    && apt install -y openssl ca-certificates curl \
    && apt clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=builder /usr/src/target/release/memex ./
ENTRYPOINT [ "./memex" ]