FROM debian:bookworm-slim
ARG SINGBOX_RELEASE_URL=https://github.com/SagerNet/sing-box/releases/download/v1.13.11/sing-box-1.13.11-linux-amd64.tar.gz
RUN apt-get update && apt-get install -y bash ca-certificates curl jq python3 tar && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /tmp/singbox-dist \
    && curl -fsSL "$SINGBOX_RELEASE_URL" -o /tmp/sing-box.tar.gz \
    && tar -xzf /tmp/sing-box.tar.gz -C /tmp/singbox-dist \
    && install -m 0755 "$(find /tmp/singbox-dist -type f -name sing-box | head -n 1)" /usr/local/bin/sing-box
WORKDIR /scripts
RUN useradd --system --create-home --home-dir /scripts --shell /usr/sbin/nologin anneal
COPY scripts/docker-e2e.sh /scripts/docker-e2e.sh
RUN chmod +x /scripts/docker-e2e.sh
USER anneal
ENTRYPOINT ["/bin/bash", "/scripts/docker-e2e.sh"]
