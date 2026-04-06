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
RUN useradd --system --create-home --home-dir /app --shell /usr/sbin/nologin anneal
COPY --from=builder /src/target/release/worker /usr/local/bin/worker
COPY migrations /app/migrations
RUN chown -R anneal:anneal /app
USER anneal
CMD ["worker"]
