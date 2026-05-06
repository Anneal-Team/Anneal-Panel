FROM rust:1.91-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p node-agent

FROM debian:bookworm-slim
ARG XRAY_RELEASE_URL=https://github.com/XTLS/Xray-core/releases/download/v26.2.6/Xray-linux-64.zip
ARG SINGBOX_RELEASE_URL=https://github.com/SagerNet/sing-box/releases/download/v1.13.11/sing-box-1.13.11-linux-amd64.tar.gz
RUN apt-get update && apt-get install -y ca-certificates curl unzip tar openssl iproute2 && rm -rf /var/lib/apt/lists/*
WORKDIR /agent
RUN useradd --system --create-home --home-dir /var/lib/anneal --shell /usr/sbin/nologin anneal
COPY --from=builder /src/target/release/node-agent /usr/local/bin/node-agent
RUN curl -fsSL "$XRAY_RELEASE_URL" -o /tmp/xray.zip \
    && mkdir -p /tmp/xray-dist /tmp/singbox-dist \
    && unzip -oq /tmp/xray.zip -d /tmp/xray-dist \
    && curl -fsSL "$SINGBOX_RELEASE_URL" -o /tmp/sing-box.tar.gz \
    && tar -xzf /tmp/sing-box.tar.gz -C /tmp/singbox-dist \
    && install -m 0755 /tmp/xray-dist/xray /usr/local/bin/xray \
    && install -m 0755 "$(find /tmp/singbox-dist -type f -name sing-box | head -n 1)" /usr/local/bin/sing-box \
    && chmod +x /usr/local/bin/xray /usr/local/bin/sing-box \
    && mkdir -p /var/lib/anneal/xray /var/lib/anneal/singbox /var/lib/anneal/tls \
    && chown -R anneal:anneal /var/lib/anneal
USER anneal
CMD ["node-agent", "--once"]
