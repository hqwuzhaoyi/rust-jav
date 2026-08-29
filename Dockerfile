# syntax=docker/dockerfile:1.7
FROM --platform=$BUILDPLATFORM node:22-bookworm-slim AS frontend
WORKDIR /src/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.88-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY rules.yaml ./rules.yaml
COPY src ./src
COPY --from=frontend /src/frontend/dist ./frontend/dist
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl gosu \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 568 rust-jav \
    && useradd --uid 568 --gid 568 --home-dir /nonexistent --no-create-home --shell /usr/sbin/nologin rust-jav
COPY --from=builder /src/target/release/rust-jav /usr/local/bin/rust-jav
COPY docker/entrypoint.sh /usr/local/bin/rust-jav-entrypoint
ENV RUST_JAV_UID=568 RUST_JAV_GID=568 RUST_JAV_CONFIG=/config/management.yaml
EXPOSE 9317
VOLUME ["/config", "/state", "/cache", "/media", "/actors"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD curl --fail --silent http://127.0.0.1:9317/health/ready || exit 1
ENTRYPOINT ["/usr/local/bin/rust-jav-entrypoint"]
CMD ["serve", "--config", "/config/management.yaml"]
