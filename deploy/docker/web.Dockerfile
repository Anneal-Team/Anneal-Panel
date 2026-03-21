FROM node:22-bookworm AS builder
WORKDIR /src
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web ./
RUN npm run build

FROM caddy:2.10-alpine
COPY deploy/docker/Caddyfile.web /etc/caddy/Caddyfile
COPY --from=builder /src/dist /srv
