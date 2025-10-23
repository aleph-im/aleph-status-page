# Use Cargo chef to cache dependencies
FROM rust:1.90-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage
FROM rust:1.90-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef

WORKDIR /usr/src/app
COPY --from=chef /usr/src/app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo install --path . --locked --bin aleph-monitor --root /out \
 && strip target/release/aleph-monitor || true

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*

# Non-root user/group
RUN groupadd -g 1001 aleph && useradd -r -u 1001 -g aleph aleph

WORKDIR /app
COPY --from=builder --chown=aleph:aleph /out/bin/aleph-monitor /app/aleph-monitor
USER aleph

ENV RUST_LOG=info
ENV ALEPH_MONITOR_PORT=8000
EXPOSE ${PORT}

ENTRYPOINT ["/app/aleph-monitor"]
