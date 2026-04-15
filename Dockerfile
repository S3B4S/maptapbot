# ─── Build stage ────────────────────────────────────────────────────────────
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies for rusqlite (bundled sqlite needs cc)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN apt-get install -y sqlite3

# Cache dependencies: copy manifests first, build a stub, then replace with
# real source. This means a code-only change doesn't re-download crates.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real binary
COPY src ./src
# Touch main.rs so cargo knows it changed
RUN touch src/main.rs && cargo build --release

# ─── Runtime stage ───────────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

# ca-certificates: needed for TLS connections to Discord's API
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/maptapbot ./maptapbot

# Default DB path — override with DATABASE_PATH env var and mount a volume
ENV DATABASE_PATH=/data/maptap.db

RUN mkdir /data

CMD ["./maptapbot"]
