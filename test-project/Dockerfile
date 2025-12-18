# Build stage: Compile the Rust application
# Runtime stage: Minimal image with only the compiled binary

# Build Stage
# -----------------------------------------------------------------------------
ARG RUST_VERSION=1.92.0
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock* ./

RUN mkdir src && \
    echo 'fn main() { println!("Placeholder"); }' > src/main.rs

RUN cargo build --release && \
    rm -rf src target/release/deps/test_project*

COPY src ./src

RUN cargo build --release

# Runtime Stage
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash app

WORKDIR /app

COPY --from=builder /app/target/release/test-project /usr/local/bin/

COPY migrations ./migrations

RUN chown -R app:app /app

USER app

EXPOSE 8080 9090

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["test-project"]
