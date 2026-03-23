FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/anneal
COPY bundle/bin/api /usr/local/bin/api
COPY bundle/migrations /opt/anneal/migrations
ENV ANNEAL_BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
CMD ["api"]
