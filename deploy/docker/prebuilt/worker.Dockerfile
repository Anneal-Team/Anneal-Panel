FROM debian:bookworm-slim AS downloader
ARG RELEASE_BASE_URL
ARG TARGET_TRIPLE=linux-amd64
RUN apt-get update && apt-get install -y ca-certificates curl tar && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /payload/bin /payload/migrations \
    && curl -fsSL "${RELEASE_BASE_URL}/worker-${TARGET_TRIPLE}.tar.gz" | tar -xzf - -C /payload/bin \
    && curl -fsSL "${RELEASE_BASE_URL}/migrations.tar.gz" | tar -xzf - -C /payload/migrations

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/anneal
COPY --from=downloader /payload/bin/worker /usr/local/bin/worker
COPY --from=downloader /payload/migrations /opt/anneal/migrations
CMD ["worker"]
