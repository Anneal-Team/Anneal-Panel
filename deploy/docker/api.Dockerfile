FROM rust:1.91-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
RUN useradd --system --create-home --home-dir /app --shell /usr/sbin/nologin anneal
COPY --from=builder /src/target/release/api /usr/local/bin/api
COPY migrations /app/migrations
RUN chown -R anneal:anneal /app
ENV ANNEAL_BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
USER anneal
CMD ["api"]
