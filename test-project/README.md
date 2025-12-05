# Test Project

A sample Rust application for testing and validating **expresso-kit** features.

## Overview

This project demonstrates a typical Rust web application setup with:
- **Rust 1.91.1** - Latest stable Rust toolchain
- **PostgreSQL 17** - Primary database with SQLx
- **Redis 7** - Caching and session storage
- **SQLx Migrations** - Database schema management
- **Docker Compose** - Container orchestration

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Rust 1.91.1 (optional, for local development)

### Using Docker Compose

```bash
# Start all services
docker compose up -d

# Start with dev tools (Adminer, Redis Commander)
docker compose --profile dev up -d

# Run migrations
docker compose run --rm migrations

# View logs
docker compose logs -f app
```

### Local Development

```bash
# Copy environment sample
cp .env.sample .env

# Start dependencies only
docker compose up -d postgres redis

# Run migrations
sqlx migrate run

# Run the application
cargo run
```

## Project Structure

```
test-project/
├── .env.sample              # Environment variables template
├── .env                     # Local environment configuration
├── docker-compose.sample.yml # Docker Compose template
├── docker-compose.yml       # Local Docker Compose configuration
├── Dockerfile               # Application container
├── Dockerfile.migrations    # Migrations container
├── Cargo.toml               # Rust dependencies
├── rust-toolchain.toml      # Rust version specification
├── src/
│   └── main.rs              # Application entry point
└── migrations/
    ├── init/                # Database initialization scripts
    │   └── 00_init.sql
    ├── 20241201000000_create_users_table.sql
    ├── 20241201000001_create_sessions_table.sql
    └── 20241201000002_create_audit_log_table.sql
```

## Environment Variables

See `.env.sample` for all available configuration options with sensible defaults.

### Key Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `APP_PORT` | 8080 | Application HTTP port |
| `DATABASE_URL` | postgres://... | PostgreSQL connection string |
| `REDIS_URL` | redis://localhost:6379 | Redis connection string |
| `JWT_SECRET` | (see sample) | JWT signing secret |
| `RUST_LOG` | debug | Log level configuration |

## Testing with expresso-kit

This project is used as the primary test fixture for **expresso-kit** CI workflows.

```bash
# Discover projects in workspace
expresso-kit discover --path .

# Full validation (env + compose + files)
expresso-kit validate --path ./test-project

# Strict validation (fails on warnings)
expresso-kit validate --path ./test-project --strict

# Environment-only validation
expresso-kit validate --path ./test-project --env-only

# Docker Compose-only validation
expresso-kit validate --path ./test-project --compose-only

# Compare docker-compose.yml with docker-compose.sample.yml
expresso-kit validate --path ./test-project --compose-only --compare

# Cross-validate env vars referenced in docker-compose
expresso-kit validate --path ./test-project --cross-validate

# Validate required files exist
expresso-kit validate --path ./test-project --files-only \
  --required-files ".env,.env.sample,docker-compose.yml,Dockerfile"

# List all services in docker-compose
expresso-kit list-services --path ./test-project

# List services with environment variable details
expresso-kit list-services --path ./test-project --with-env

# Generate CI workflow for this project
expresso-kit init-workflow --path ./test-project --dry-run

# Check system dependencies
expresso-kit check-deps
```

### Output Formats

All commands support multiple output formats:

```bash
# Human-readable text (default)
expresso-kit validate --path ./test-project --format text

# JSON for parsing/automation
expresso-kit validate --path ./test-project --format json

# GitHub Actions annotations
expresso-kit validate --path ./test-project --format github
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Hello message |
| GET | `/health` | Health check endpoint |

## License

MIT
