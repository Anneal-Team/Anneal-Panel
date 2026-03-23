FROM rust:1.91-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p node-agent

FROM debian:bookworm-slim
ARG XRAY_RELEASE_URL=https://github.com/XTLS/Xray-core/releases/download/v26.2.6/Xray-linux-64.zip
ARG SINGBOX_RELEASE_URL=https://github.com/hiddify/hiddify-core/releases/download/v4.0.4/hiddify-core-linux-amd64.tar.gz
RUN apt-get update && apt-get install -y ca-certificates curl unzip tar openssl iproute2 && rm -rf /var/lib/apt/lists/*
WORKDIR /agent
COPY --from=builder /src/target/release/node-agent /usr/local/bin/node-agent
RUN curl -fsSL "$XRAY_RELEASE_URL" -o /tmp/xray.zip \
    && mkdir -p /tmp/xray-dist /tmp/singbox-dist \
    && unzip -oq /tmp/xray.zip -d /tmp/xray-dist \
    && curl -fsSL "$SINGBOX_RELEASE_URL" -o /tmp/hiddify-core.tar.gz \
    && tar -xzf /tmp/hiddify-core.tar.gz -C /tmp/singbox-dist \
    && install -m 0755 /tmp/xray-dist/xray /usr/local/bin/xray \
    && install -m 0755 "$(find /tmp/singbox-dist -type f -name hiddify-core | head -n 1)" /usr/local/bin/hiddify-core \
    && chmod +x /usr/local/bin/xray /usr/local/bin/hiddify-core \
    && mkdir -p /var/lib/anneal/xray /var/lib/anneal/singbox /var/lib/anneal/tls
CMD ["node-agent", "--once"]
