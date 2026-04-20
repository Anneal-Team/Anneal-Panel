FROM debian:bookworm

ENV container=docker
ENV ANNEAL_GITHUB_REPOSITORY=Anneal-Team/Anneal-Panel
ENV ANNEAL_RELEASE_TAG=rolling-master
ENV ANNEAL_INSTALLER_LANG=ru

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        dbus \
        iproute2 \
        openssl \
        procps \
        systemd \
        systemd-sysv \
        tar \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask \
        dev-hugepages.mount \
        sys-fs-fuse-connections.mount \
        systemd-logind.service \
        getty.target \
        getty@tty1.service

COPY scripts/install.sh /usr/local/bin/anneal-install-test

RUN sed -i 's/\r$//' /usr/local/bin/anneal-install-test \
    && chmod +x /usr/local/bin/anneal-install-test \
    && printf '%s\n' \
        '#!/usr/bin/env bash' \
        'set -euo pipefail' \
        'echo "Anneal VPS test container is running."' \
        'echo "Open installer with: docker exec -it ${HOSTNAME} anneal-install-test"' \
        'echo "Use Docker deployment mode for the closest container-safe VPS test."' \
        'exec /sbin/init' \
        > /usr/local/bin/anneal-vps-test \
    && chmod +x /usr/local/bin/anneal-vps-test

STOPSIGNAL SIGRTMIN+3
VOLUME ["/sys/fs/cgroup", "/var/lib/anneal", "/etc/anneal", "/var/lib/caddy"]
CMD ["/usr/local/bin/anneal-vps-test"]
