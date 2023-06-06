FROM rust:1.68 as builder

WORKDIR /usr/src
# Need for a successful whisper-rs build for some reason...
RUN rustup component add rustfmt
# cmake/clang required for llama-rs/whisper-rs builds
RUN apt update -y && apt upgrade -y
RUN apt install build-essential -y \
    cmake \
    clang

COPY . .
RUN cargo build -p sightglass --release

FROM debian:stable-slim
ENV WHISPER_MODEL_SMALL=https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
# ENV WHISPER_MODEL_MEDIUM=https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin
# ENV WHISPER_MODEL_LARGE=https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v1.bin

WORKDIR /app
RUN apt update \
    && apt install -y openssl ca-certificates curl \
    && apt clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Download whisper model used in development
RUN mkdir -p models \
    && curl -L --output models/whisper.small.bin ${WHISPER_MODEL_SMALL}
    # && curl -L --output models/whisper.medium.bin ${WHISPER_MODEL_MEDIUM} \
    # && curl -L --output models/whisper.large.bin ${WHISPER_MODEL_LARGE}

COPY --from=builder /usr/src/target/release/sightglass ./

EXPOSE 8080
ENTRYPOINT [ "./sightglass" ]