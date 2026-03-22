FROM debian:bookworm-slim AS downloader
ARG RELEASE_BASE_URL
ARG TARGET_TRIPLE=linux-amd64
RUN apt-get update && apt-get install -y ca-certificates curl tar && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /payload/bin /payload/runtime \
    && curl -fsSL "${RELEASE_BASE_URL}/node-agent-${TARGET_TRIPLE}.tar.gz" | tar -xzf - -C /payload/bin \
    && curl -fsSL "${RELEASE_BASE_URL}/runtime-bundle-${TARGET_TRIPLE}.tar.gz" | tar -xzf - -C /payload/runtime

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates iproute2 openssl supervisor && rm -rf /var/lib/apt/lists/*
WORKDIR /var/lib/anneal
COPY --from=downloader /payload/bin/node-agent /usr/local/bin/node-agent
COPY --from=downloader /payload/runtime/xray /usr/local/bin/xray
COPY --from=downloader /payload/runtime/hiddify-core /usr/local/bin/hiddify-core
COPY node-supervisord.conf /etc/supervisor/conf.d/anneal-node.conf
RUN mkdir -p /var/lib/anneal/xray /var/lib/anneal/singbox /var/lib/anneal/tls
ENV ANNEAL_AGENT_CONFIG_ROOT=/var/lib/anneal
ENV ANNEAL_AGENT_RUNTIME_CONTROLLER=supervisorctl
ENV ANNEAL_AGENT_SYSTEMCTL_BINARY=/usr/bin/supervisorctl
ENV ANNEAL_AGENT_XRAY_SERVICE=xray
ENV ANNEAL_AGENT_SINGBOX_SERVICE=singbox
CMD ["/usr/bin/supervisord", "-n", "-c", "/etc/supervisor/supervisord.conf"]
