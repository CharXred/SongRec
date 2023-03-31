# Builder
FROM rust:slim-buster as builder
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    --allow-unauthenticated \
    build-essential pkg-config libasound2-dev libgtk-3-dev libssl-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
WORKDIR /app/
COPY . .
RUN cargo build --manifest-path chartex-radio/Cargo.toml --release --verbose
RUN mkdir -p build-out && \
    cp chartex-radio/target/release/chartex-radio build-out/

# Runtime
FROM debian:buster-slim as runtime-image
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    --allow-unauthenticated \
    libasound2-dev libgtk-3-dev ca-certificates ffmpeg \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
WORKDIR /app/
COPY --from=builder /app/chartex-radio/log4rs.yml .
COPY --from=builder /app/build-out/chartex-radio .
ENTRYPOINT [ "./chartex-radio" ]
