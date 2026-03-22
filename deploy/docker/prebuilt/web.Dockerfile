FROM alpine:3.20 AS downloader
ARG RELEASE_BASE_URL
RUN apk add --no-cache curl tar
RUN mkdir -p /payload/web \
    && curl -fsSL "${RELEASE_BASE_URL}/web.tar.gz" | tar -xzf - -C /payload/web

FROM caddy:2.10-alpine
COPY --from=downloader /payload/web /srv
