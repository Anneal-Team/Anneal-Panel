FROM rust:1.91-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p worker

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/worker /usr/local/bin/worker
COPY migrations /app/migrations
CMD ["worker"]
