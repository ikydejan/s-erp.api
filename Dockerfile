# syntax=docker/dockerfile:1
# Build with: docker build --build-arg BIN_NAME=api -t s-erp-api .
#             docker build --build-arg BIN_NAME=worker -t s-erp-worker .
#
# Note: this used to use cargo-chef for a separate dependency-caching layer,
# but cargo-chef panics internally (cargo-chef 0.1.78, recipe.rs:224) when
# cooking this workspace's recipe.json, so it was dropped in favor of a
# plain `cargo build`. Layers still get Docker/BuildKit `type=gha` caching
# via the build-push-action step, just without the finer dependency-only
# cache split chef would normally provide.

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
ARG BIN_NAME
RUN cargo build --release --bin ${BIN_NAME} \
    && cp target/release/${BIN_NAME} /app/app

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -u 10001 -m appuser
WORKDIR /app
COPY --from=builder /app/app /app/app
USER appuser
EXPOSE 8080
ENTRYPOINT ["/app/app"]
