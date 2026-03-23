FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates iproute2 openssl supervisor && rm -rf /var/lib/apt/lists/*
WORKDIR /var/lib/anneal
COPY bundle/bin/node-agent /usr/local/bin/node-agent
COPY bundle/runtime/xray /usr/local/bin/xray
COPY bundle/runtime/hiddify-core /usr/local/bin/hiddify-core
COPY node-supervisord.conf /etc/supervisor/conf.d/anneal-node.conf
RUN mkdir -p /var/lib/anneal/xray /var/lib/anneal/singbox /var/lib/anneal/tls
ENV ANNEAL_AGENT_CONFIG_ROOT=/var/lib/anneal
ENV ANNEAL_AGENT_RUNTIME_CONTROLLER=supervisorctl
ENV ANNEAL_AGENT_SYSTEMCTL_BINARY=/usr/bin/supervisorctl
ENV ANNEAL_AGENT_XRAY_SERVICE=xray
ENV ANNEAL_AGENT_SINGBOX_SERVICE=singbox
CMD ["/usr/bin/supervisord", "-n", "-c", "/etc/supervisor/supervisord.conf"]
