FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/anneal
COPY bundle/bin/worker /usr/local/bin/worker
COPY bundle/migrations /opt/anneal/migrations
CMD ["worker"]
