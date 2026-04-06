FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/anneal
RUN useradd --system --create-home --home-dir /opt/anneal --shell /usr/sbin/nologin anneal
COPY bundle/bin/worker /usr/local/bin/worker
COPY bundle/migrations /opt/anneal/migrations
RUN chown -R anneal:anneal /opt/anneal
USER anneal
CMD ["worker"]
