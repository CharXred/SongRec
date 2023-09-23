FROM rust:1.72.0-alpine3.18 as builder
WORKDIR /app
RUN apk update
RUN apk add --no-cache musl-dev
COPY Cargo.toml Cargo.toml
RUN mkdir -p src/
RUN echo "fn main() {println!(\"failed to build\")}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/radio_server*
COPY . .
RUN cargo build --locked --release
RUN mkdir -p build-out/
RUN cp target/release/radio-server build-out/

FROM scratch
WORKDIR /app
COPY --from=builder /app/build-out/radio-server .
EXPOSE 3000
USER 1000:1000
CMD ["./radio-server"]
