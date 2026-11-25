# syntax=docker/dockerfile:1
# Build with: docker build --build-arg BIN_NAME=api -t s-erp-api .
#             docker build --build-arg BIN_NAME=worker -t s-erp-worker .

FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
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
