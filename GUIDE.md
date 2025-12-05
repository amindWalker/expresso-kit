# LXP Docker Compose + Uniconn Setup Guide

This guide uses **expresso-kit** to debug, validate, and automate fixing common setup issues in the LXP Docker Compose environment.

## TL;DR - Quick Fix Checklist

For experienced developers who just need a quick introduction to the `CLI` (`TUI` version at the bottom):

### macOS/Linux/Windows WSL (Bash/Zsh/Fish)

```bash
# 1. Navigate to lxp-docker-compose
cd projects/lxp-docker-compose

# 2. Validate everything with expresso-kit
expresso-kit validate --cross-validate

# 3. Check if .env exists (copy from env.sample if missing)
test -f .env || cp env.sample .env

# 4. Fill in secrets from LastPass "LXP Config Secrets - NONPROD"
# Edit .env with your editor (nano, vim, code, etc.)

# 5. Login to JFrog (required to pull Docker images)
docker login iseatz-lxp-docker.jfrog.io -u "YOUR_EMAIL" -p "YOUR_JFROG_API_KEY"

# 5.1 Setup each LXP service in repositories directory according to their README.md

# 6. Build and run the services
docker compose --profile lxp build
docker compose up postgres redis lxp-uniconn --abort-on-container-exit
```

### Windows (PowerShell if not using WSL)

```powershell
# 1. Navigate to lxp-docker-compose
cd projects\lxp-docker-compose

# 2. Validate everything with expresso-kit
expresso-kit validate --cross-validate

# 3. Check if .env exists (copy from env.sample if missing)
if (-not (Test-Path .env)) { Copy-Item env.sample .env }

# 4. Fill in secrets from LastPass "LXP Config Secrets - NONPROD"
# Edit .env with your editor (notepad, code, etc.)

# 5. Login to JFrog (required to pull Docker images)
docker login iseatz-lxp-docker.jfrog.io -u "YOUR_EMAIL" -p "YOUR_JFROG_API_KEY"

# 5.1 Setup each LXP service in repositories directory according to their README.md

# 6. Build and run the services
docker compose --profile lxp build
docker compose up postgres redis lxp-uniconn --abort-on-container-exit
```

## Table of Contents

1. [Prerequisites Installation](#1-prerequisites-installation)
   - [Windows Setup](#windows-setup)
   - [macOS Setup](#macos-setup)
   - [Linux Setup](#linux-setup)
2. [Environment Setup](#2-environment-setup)
3. [Docker Compose Validation](#3-docker-compose-validation)
4. [Uniconn-Specific Setup](#4-uniconn-specific-setup)
5. [Database Initialization](#5-database-initialization)
6. [Common Errors & Fixes](#6-common-errors--fixes)
7. [Running the Stack](#7-running-the-stack)
8. [Using TUI Mode](#8-using-tui-mode)

## 1. Prerequisites Installation

Before starting, you need these tools installed on your system:

| Tool | Purpose |
|------|---------|
| **Git** | Clone repositories |
| **Docker** | Run containerized services |
| **Docker Compose** | Orchestrate multiple containers |
| **Rust** (optional) | Build Uniconn locally |

### Windows Setup
For Windows it is recommended to use `WSL2` (_Windows Subsystem for Linux_) for the best Docker experience.

#### Step 1: Install WSL2

If WSL is not already installed, follow these steps:

```powershell
# Open PowerShell as Administrator and run:
wsl --install
```

- If prompted that WSL is not recognized, first enable the required features:
    ```powershell
    dism.exe /online /enable-feature /featurename:Microsoft-Windows-Subsystem-Linux /all /norestart
    dism.exe /online /enable-feature /featurename:VirtualMachinePlatform /all /norestart
    ```

- Then run:
    ```powershell
    wsl --install
    ```

- Reboot your computer when prompted.

- After reboot, on first launch, you may be asked to create a Linux username and password, follow the prompts.

- To verify `WSL2` is working, open your `WSL` terminal (e.g., Ubuntu) and run:
    ```bash
    uname -a
    ```
    The output should mention "Linux" with its kernel version.


#### Step 2: Install Tools with winget

`winget` is Windows' built-in package manager (available on most modern Windows 10/11).
It is the closest `cli` package manager that is similar to Ubuntu/Debian `apt` and macOS `brew`.

```powershell
# Open PowerShell and run:

# Install Git
winget install Git.Git

# Install Docker Desktop (includes Docker Compose)
winget install Docker.DockerDesktop

# Install Windows Terminal (recommended)
winget install Microsoft.WindowsTerminal

# (Optional but recommended for local development) Install Rust
winget install Rustlang.Rustup
```

#### Step 3: Configure Docker Desktop

1. Open **Docker Desktop**
2. Go to **Settings** > **General**
3. Enable "Use the WSL 2 based engine"
4. Go to **Settings** > **Resources** > **WSL Integration**
5. Enable integration with your Linux distro (e.g., Ubuntu)
6. Click **Apply & Restart**

#### Step 4: Verify Installation (in WSL terminal)

```bash
# Open Windows Terminal and select your WSL distro (Ubuntu)
git --version      # Should show git version 2.x.x
docker --version   # Should show Docker version 24.x.x
docker compose version  # Should show Docker Compose version v2.x.x
```

### macOS Setup

macOS users can use `brew` (Homebrew) for easy installation.

#### Step 1: Install Homebrew (if not installed)

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

#### Step 2: Install Tools

```bash
# Install Git
brew install git

# Install Docker Desktop (includes Docker Compose)
brew install --cask docker

# (Optional) Install Rust
brew install rustup-init && rustup-init
```

#### Step 3: Start Docker Desktop

1. Open **Docker Desktop** from Applications
2. Complete the initial setup wizard
3. Ensure Docker is running (whale icon in menu bar)

#### Step 4: Verify Installation

```bash
git --version           # git version 2.x.x
docker --version        # Docker version 24.x.x
docker compose version  # Docker Compose version v2.x.x
```

### Linux Setup

Instructions for Ubuntu/Debian. Adapt for other distributions.

#### Step 1: Install Git

```bash
sudo apt update
sudo apt install -y git
```

#### Step 2: Install Docker

```bash
# Remove old versions
sudo apt remove docker docker-engine docker.io containerd runc

# Install prerequisites
sudo apt install -y ca-certificates curl gnupg lsb-release

# Add Docker's official GPG key
sudo mkdir -p /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg

# Add Docker repository
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

# Install Docker
sudo apt update
sudo apt install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Add your user to docker group (avoid sudo for docker commands)
sudo usermod -aG docker $USER

# Log out and back in for group changes to take effect
```

#### Step 3: (Optional) Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Step 4: Verify Installation

```bash
git --version           # git version 2.x.x
docker --version        # Docker version 24.x.x
docker compose version  # Docker Compose version v2.x.x
```

### Validate Prerequisites with expresso-kit

Once everything is installed, use expresso-kit to verify:

```bash
# Check all required tools
expresso-kit check-deps --format text

# Expected output:
# 🔍 System Dependencies
# ─────────────────────────────────────────
#   ✓ git (version 2.x.x)
#   ✓ docker (version 24.x.x)
#   ✓ docker-compose (version 2.x.x)

# Strict mode will exit with code 1 if anything is missing
expresso-kit check-deps --strict
```

## 2. Environment Setup

The LXP Docker Compose project requires several configuration files and secrets to run properly. This section walks you through setting everything up.

### Understanding the Project Structure

```
lxp-docker-compose/
├── .env                          # Your local secrets (create this)
├── env.sample                    # Template for .env
├── docker-compose.yml            # Main compose file
├── docker-compose.override.yml   # Your local customizations (create this)
├── docker-compose.override-sample.yml  # Template for override
├── shared/
│   └── localstack/
│       └── secrets.psv           # AWS secrets for LocalStack (create this)
└── repositories/
    ├── lxp-uniconn/              # Uniconn source code
    ├── lxp-gibraltar/            # Other LXP services...
    └── ...
```

### Step 2.1: Clone the Repositories

First, populate the `repositories` directory with all LXP repositories:

```bash
cd projects/lxp-docker-compose/repositories

# Clone Uniconn (required for this guide)
git clone https://github.com/iSeatz/lxp-uniconn.git

# Or use expresso-kit:
expresso-kit clone https://github.com/iSeatz/lxp-uniconn --dest lxp-uniconn
```

### Step 2.2: Create the .env File

The `.env` file contains all secrets and configuration values.

```bash
cd projects/lxp-docker-compose

# Validate if .env exists
expresso-kit validate --path . --env-only

# If .env is missing, copy from sample
cp env.sample .env
```

### Step 2.3: Fill in Required Secrets

Open `.env` in your editor and fill in these **critical values**:

#### NPM Authentication (for JFrog Artifactory)

1. Log in to [JFrog Artifactory](https://iseatz.jfrog.io) using **JumpCloud**
2. Navigate to: https://iseatz.jfrog.io/ui/repos/tree/General/lxp-npm-group
3. Click **"Set Me Up"** to get your `NPM_AUTH` token
4. Add to `.env`:
   ```bash
   NPM_AUTH=your_base64_token_here
   NPM_EMAIL=your.email@company.com
   ```

#### Database Credentials

These are the default local development values (already in env.sample):

```bash
# PostgreSQL
DATABASE_HOSTNAME=postgres
DATABASE_USERNAME=postgres
DATABASE_PASSWORD=postgres

# Uniconn-specific
DATABASE_URL=postgres://postgres:postgres@localhost:5432/lxp_uniconn_local
```

#### AWS/LocalStack Configuration

For local development with LocalStack:

```bash
AWS_ACCESS_KEY_ID=foo
AWS_SECRET_ACCESS_KEY=bar
AWS_ENDPOINT_URL=http://localhost:4566
AWS_REGION=us-east-1
```

### Step 2.4: Set Up LocalStack Secrets

Copy secrets for AWS Secrets Manager simulation:

1. Open **LastPass** and find note: **"LXP Config Secrets - NONPROD"**
2. Copy contents to: `shared/localstack/secrets.psv`

```bash
# Create the file
touch shared/localstack/secrets.psv

# Paste the contents from LastPass
# The file format is: key|value (pipe-separated)
```

### Step 2.5: Create docker-compose.override.yml (Optional)

Customize your local setup:

```bash
# Copy the sample override file
cp docker-compose.override-sample.yml docker-compose.override.yml

# Edit as needed (this file is gitignored)
```

### Step 2.6: Validate Environment with expresso-kit

```bash
# Full environment validation
expresso-kit validate --path . --env-only

# Expected output:
# ✓ Repository Validation: .
# ─────────────────────────────────────────
# Environment:
#   ✓ env.sample exists
#   ✓ .env exists
#   Missing variables: (list if any)
#   Empty variables: (list if any)

# Cross-validate env vars with docker-compose references
expresso-kit validate --cross-validate --format text
```

### Critical Environment Variables Reference

| Variable | Purpose | Example Value |
|----------|---------|---------------|
| `DATABASE_URL` | PostgreSQL connection for Uniconn | `postgres://postgres:postgres@postgres:5432/lxp_uniconn_local` |
| `AWS_ELASTICACHE_HOST` | Redis connection | `redis://redis` (docker) or `redis://localhost` (host) |
| `AWS_ENDPOINT_URL` | LocalStack endpoint | `http://localhost:4566` |
| `SQLX_OFFLINE` | SQLx offline mode (skip DB during build) | `true` |
| `UNICONN_ENV` | Environment name | `local` |
| `RUST_LOG` | Logging level | `uniconn=debug,sqlx=debug,hyper=info` |
| `NPM_AUTH` | JFrog npm authentication | Base64 token from JFrog |
| `LAUNCHDARKLY_SDK_KEY` | Feature flags | `sdk-xxxxx` (get from team) |

## 3. Docker Authentication & Build

Before running containers, you need to authenticate with JFrog to pull private Docker images.

### Step 3.1: Get Your JFrog API Key

1. Log in to [JFrog Artifactory](https://iseatz.jfrog.io) using **JumpCloud**
2. Click your username (top-right) → **Edit Profile**
3. Scroll to **Authentication Settings**
4. Generate or copy your **API Key**

### Step 3.2: Docker Login

```bash
# Replace with your actual credentials
docker login iseatz-lxp-docker.jfrog.io -u "your.email@company.com" -p "YOUR_JFROG_API_KEY"

# Expected output:
# Login Succeeded
```

> 💡 **Tip**: You only need to do this once. Docker stores credentials in `~/.docker/config.json`

### Step 3.3: Validate Docker Compose File

```bash
cd projects/lxp-docker-compose

# Validate docker-compose configuration with expresso-kit
expresso-kit validate --compose-only --path .

# Expected output:
# Docker Compose:
#   ✓ docker-compose.yml exists
#   ✓ X services defined
#   Services: postgres, redis, lxp-uniconn, ...
```

### Step 3.4: List All Available Services

```bash
# See all services defined
expresso-kit list-services --format text

# For detailed env var inspection
expresso-kit list-services --with-env --format text

# Filter specific services (requires jq)
expresso-kit list-services --with-env --format json | jq '.services[] | select(.name | test("uniconn|postgres|redis"))'
```

### Step 3.5: Build the Docker Images

```bash
# Build all LXP services
docker compose --profile lxp build

# Or build only what you need
docker compose build postgres redis lxp-uniconn

# Watch for errors during build
# Common issues:
# - NPM_AUTH not set → JFrog authentication failed
# - Network timeout → Check VPN connection
```

### Step 3.6: Understand Service Dependencies

The `lxp-uniconn` service has these dependencies defined in `docker-compose.yml`:

```yaml
lxp-uniconn:
  depends_on:
    redis:
      condition: service_started      # Redis must be running
    postgres:
      condition: service_healthy      # PostgreSQL must pass health check
```

This means when you start `lxp-uniconn`, Docker Compose will automatically start `redis` and `postgres` first.

## 4. Uniconn-Specific Setup

Uniconn is a **Rust application** that requires additional setup beyond the Docker Compose environment.

### What is Uniconn?

- Handles **all external supplier API integrations** (hotels, cars, dining, activities)
- Stores **catalog data** in PostgreSQL
- Uses **Redis** for caching
- Uses **SQLx** for type-safe database queries
- Runs on port **3000** by default

### Step 4.1: Ensure Uniconn Repository Exists

```bash
cd projects/lxp-docker-compose/repositories

# Check if lxp-uniconn exists
ls -la lxp-uniconn/

# If missing, clone it:
git clone https://github.com/iSeatz/lxp-uniconn.git

# Or use expresso-kit:
expresso-kit clone https://github.com/iSeatz/lxp-uniconn --dest lxp-uniconn

# Validate the cloned repo
expresso-kit validate --path lxp-uniconn --cross-validate
```

### Step 4.2: Uniconn Environment File

Uniconn needs its **own `.env` file** in addition to the root `.env`:

```bash
cd repositories/lxp-uniconn

# Check if .env exists
expresso-kit validate --env-only

# If missing, you need secrets from LastPass:
# 1. Open LastPass
# 2. Find note: "SECRETS .ENV" (for Uniconn)
# 3. Copy contents to: repositories/lxp-uniconn/.env
```

### Step 4.3: LXP Configuration File

Uniconn uses YAML configuration files in `lxp_config/` directory:

```bash
ls repositories/lxp-uniconn/lxp_config/
# local.yml    ← Used for local development
# dev.yml
# staging.yml
# prod.yml
```

The `local.yml` file maps secrets from environment variables. Make sure your `.env` has all required keys.

### Step 4.4: Database Migrations (for local Rust development)

If you're building Uniconn locally (not via Docker), run migrations:

```bash
cd repositories/lxp-uniconn

# Run the setup script (installs deps + migrations)
./scripts/setup.sh

# Or manually run migrations
./scripts/migrate.sh
```

### Step 4.5: SQLx Offline Mode

Uniconn uses **SQLx offline mode** to avoid requiring a database during compilation:

```bash
# The .sqlx directory contains cached query metadata
ls repositories/lxp-uniconn/.sqlx/

# If building fails with "offline data not found", regenerate:
cd repositories/lxp-uniconn
cargo sqlx prepare

# This requires a running PostgreSQL with migrations applied
```

> ⚠️ **Important**: When using Docker Compose, `SQLX_OFFLINE=true` is set automatically. You only need to worry about this for local development.

## 5. Database Initialization

PostgreSQL is the backbone of Uniconn's data storage. This section covers starting it correctly and loading data.

### Step 5.1: Start PostgreSQL

```bash
cd projects/lxp-docker-compose

# Start only PostgreSQL
docker compose up -d postgres

# Watch the logs to ensure it starts correctly
docker compose logs -f postgres
```

### Step 5.2: Wait for Health Check

PostgreSQL has a **health check** configured. Uniconn won't start until Postgres is healthy:

```bash
# Check PostgreSQL status
docker compose ps postgres

# Expected output:
# NAME       STATUS            PORTS
# postgres   running (healthy) 0.0.0.0:5432->5432/tcp

# If it shows "starting" or "(health: starting)", wait a few seconds
# If it shows "unhealthy", check logs:
docker compose logs postgres --tail 50
```

### Step 5.3: Verify Database Connectivity

```bash
# Test connection from host
docker compose exec postgres pg_isready
# Expected: /var/run/postgresql:5432 - accepting connections

# Connect to PostgreSQL CLI
docker compose exec postgres psql -U postgres

# Inside psql, list databases:
\l

# You should see lxp_uniconn_local (created automatically)
# Exit with: \q
```

### Step 5.4: Create Database Manually (if needed)

If the `lxp_uniconn_local` database doesn't exist:

```bash
# Create the database
docker compose exec postgres createdb -U postgres lxp_uniconn_local

# Verify it was created
docker compose exec postgres psql -U postgres -c "\l" | grep uniconn
```

### Step 5.5: Load Environment Variables for Migrations

Before running migrations, load your `.env` into the shell:

```bash
# For Bash/Zsh:
set -a; source .env; set +a

# For Fish shell:
export (grep -v '^#' .env | xargs -L 1)

# Run the init script (migrates databases for all services)
./init-env.sh
```

### Step 5.6: Loading a Database Dump (Optional but Recommended)

Uniconn can load catalog data from suppliers, but this takes **hours or days** (especially for Viator). It's faster to load from a database dump.

> ⚠️ **WARNING**: This will **delete all existing data** in your database!

```bash
# 1. Stop PostgreSQL and remove its volume
docker compose down -v postgres

# 2. Get a dump file from your team (usually named like: uniconn.pg.YYYY.MM.DD.sql.gz)

# 3. Load the compressed dump (keep it gzipped!)
./load-uniconn-compressed-db.sh uniconn.pg.2024.09.28.sql.gz

# 4. Start PostgreSQL fresh
docker compose up -d postgres

# 5. Wait for initialization (10-20 minutes for large dumps)
docker compose logs -f postgres

# Look for: "database system is ready to accept connections"
```

### Step 5.7: Seeding Data Without a Dump

If you don't have a dump, you can load data from suppliers:

```bash
# Start the full stack first
docker compose up -d postgres redis lxp-uniconn

# Wait for uniconn to be ready
curl http://localhost:3000/health

# Run catalog loaders (from uniconn directory)
cd repositories/lxp-uniconn

# Load all suppliers (WARNING: takes hours/days)
./scripts/catalog_loaders/load_all.sh

# Or load specific suppliers
./scripts/catalog_loaders/load_opentable.sh   # Dining
./scripts/catalog_loaders/load_cartrawler.sh  # Cars
# Note: Viator (activities) takes the longest
```

## 6. Common Errors & Fixes

This section covers the most frequent issues developers encounter, with detailed diagnosis and solutions.

### Error: "Cannot pull image" / "JFrog authentication failed"

**Symptoms:**
```
Error response from daemon: pull access denied for iseatz-lxp-docker.jfrog.io/...
```

**Diagnosis:**
```bash
# Check if you're logged in
docker login iseatz-lxp-docker.jfrog.io
```

**Fix:**
```bash
# Get your API key from JFrog (see Step 3.1)
docker login iseatz-lxp-docker.jfrog.io -u "your.email@company.com" -p "YOUR_API_KEY"
```

### Error: "Missing environment variable X"

**Symptoms:**
```
Error: environment variable 'DATABASE_URL' not set
```

**Diagnosis:**
```bash
expresso-kit validate --cross-validate --format text

# Or check specific service
expresso-kit list-services --with-env | grep -A 30 "lxp-uniconn"
```

**Fix:**
```bash
# Add missing variable to .env
echo "DATABASE_URL=postgres://postgres:postgres@postgres:5432/lxp_uniconn_local" >> .env

# Reload environment
set -a; source .env; set +a
```

### Error: "Cannot connect to PostgreSQL"

**Symptoms:**
```
connection refused (os error 111)
FATAL: password authentication failed for user "postgres"
```

**Diagnosis:**
```bash
# Check if postgres is running and healthy
docker compose ps postgres

# Check postgres logs
docker compose logs postgres --tail 50

# Test connection
docker compose exec postgres pg_isready
```

**Fix:**

1. **If PostgreSQL isn't running:**
   ```bash
   docker compose up -d postgres
   ```

2. **If health check fails:**
   ```bash
   # Remove and recreate
   docker compose down -v postgres
   docker compose up -d postgres
   ```

3. **Wrong hostname in DATABASE_URL:**
   ```bash
   # Inside Docker network, use service name:
   DATABASE_URL=postgres://postgres:postgres@postgres:5432/lxp_uniconn_local

   # From host machine (localhost):
   DATABASE_URL=postgres://postgres:postgres@localhost:5432/lxp_uniconn_local
   ```

### Error: "Cannot connect to Redis"

**Symptoms:**
```
redis connection failed
Error connecting to Redis at redis:6379
```

**Diagnosis:**
```bash
# Check Redis status
docker compose ps redis

# Test Redis connection
docker compose exec redis redis-cli ping
# Should return: PONG

# Check logs
docker compose logs redis --tail 20
```

**Fix:**
```bash
# Start Redis if not running
docker compose up -d redis

# Verify AWS_ELASTICACHE_HOST in .env:
# Inside Docker network:
AWS_ELASTICACHE_HOST=redis://redis

# From host machine:
AWS_ELASTICACHE_HOST=redis://localhost
```

### Error: "SQLX offline data not found"

**Symptoms:**
```
error: failed to find data for query
```

**Diagnosis:**
```bash
# Check if .sqlx directory exists
ls -la repositories/lxp-uniconn/.sqlx/

# Should contain .json files for each query
```

**Fix:**
```bash
cd repositories/lxp-uniconn

# Ensure SQLX_OFFLINE=true in .env
echo "SQLX_OFFLINE=true" >> .env

# If you need to regenerate (requires running Postgres with migrations):
cargo sqlx prepare
```

> 💡 **Note**: The `.sqlx` directory should be committed to git. If it's missing, pull the latest from the repository.

### Error: "Port already in use"

**Symptoms:**
```
Bind for 0.0.0.0:5432 failed: port is already allocated
```

**Diagnosis:**
```bash
# Check what's using the port
# macOS/Linux:
lsof -i :5432

# Windows (PowerShell):
netstat -ano | findstr :5432
```

**Fix:**
```bash
# Option 1: Stop the conflicting service
# If it's a local PostgreSQL:
brew services stop postgresql  # macOS
sudo systemctl stop postgresql  # Linux

# Option 2: Change the port in docker-compose.override.yml
echo 'services:
  postgres:
    ports:
      - "5433:5432"' > docker-compose.override.yml

# Then use port 5433 in your DATABASE_URL
```

### Error: "LocalStack not ready"

**Symptoms:**
```
Could not connect to http://localhost:4566
AWS endpoint not available
```

**Diagnosis:**
```bash
docker compose ps localstack
docker compose logs localstack --tail 50
```

**Fix:**
```bash
# Start LocalStack
docker compose up -d localstack

# Wait for it to be healthy (can take 30-60 seconds)
docker compose logs -f localstack
# Look for: "Ready."

# Check secrets were loaded
docker compose exec localstack awslocal secretsmanager list-secrets
```

### Error: "NPM_AUTH not set" (during build)

**Symptoms:**
```
npm ERR! 401 Unauthorized
npm ERR! code E401
```

**Fix:**
```bash
# Get NPM_AUTH from JFrog:
# 1. Login to https://iseatz.jfrog.io
# 2. Go to: Artifacts → lxp-npm-group → "Set Me Up"
# 3. Copy the base64 token

# Add to .env:
NPM_AUTH=your_base64_token_here
NPM_EMAIL=your.email@company.com

# Rebuild
docker compose build --no-cache lxp-uniconn
```

## 7. Running the Stack

You're almost there! This section covers the final steps to get everything running.

### Step 7.1: Final Validation

Before starting, run a complete validation:

```bash
cd projects/lxp-docker-compose

# Comprehensive validation
expresso-kit validate --cross-validate --format text

# Check for any remaining issues
expresso-kit validate --strict
# Exit code 0 = all good!
# Exit code 1 = issues found (check output)
```

### Step 7.2: Start the Services

**Option A: Foreground Mode (Recommended for debugging)**

See all logs in real-time. If any container fails, all containers stop:

```bash
docker compose up postgres redis lxp-uniconn --abort-on-container-exit
```

**Option B: Detached Mode**

Run in background:

```bash
# Start services
docker compose up -d postgres redis lxp-uniconn

# View logs separately
docker compose logs -f lxp-uniconn
```

**Option C: Start Everything (Full LXP Stack)**

```bash
# Build everything first
docker compose --profile lxp build

# Start all services
docker compose --profile lxp up -d
```

### Step 7.3: Verify Everything is Running

```bash
# Check all services
docker compose ps

# Expected output:
# NAME           STATUS            PORTS
# postgres       running (healthy) 0.0.0.0:5432->5432/tcp
# redis          running (healthy) 0.0.0.0:6379->6379/tcp
# lxp-uniconn    running           0.0.0.0:3000->3000/tcp
```

### Step 7.4: Test Uniconn Endpoints

```bash
# Health check
curl http://localhost:3000/health
# Expected: {"status":"ok"} or similar

# If curl isn't available (Windows):
# Open browser to: http://localhost:3000/health

# View real-time logs
docker compose logs -f lxp-uniconn
```

### Step 7.5: Testing with Postman

1. Open **Postman**
2. Import the LXP collection (ask team for file)
3. Select **"LOCAL"** environment
4. Test endpoints against `http://localhost:3000`

### Step 7.6: Monitoring with Dozzle (Optional)

Dozzle provides a web UI for viewing container logs:

```bash
# Start Dozzle
docker compose up -d dozzle

# Open in browser
open http://localhost:8888  # macOS
xdg-open http://localhost:8888  # Linux
start http://localhost:8888  # Windows
```

### Step 7.7: OpenTelemetry/Jaeger (Optional)

For distributed tracing:

```bash
# Start Jaeger
docker compose up -d jaeger otel

# Open Jaeger UI
open http://localhost:16686
```

Configure in `.env`:
```bash
OTEL_PROTOCOL=http-protobuf
OTEL_ENDPOINT=http://localhost:4318
OTEL_SERVICE_NAME=uniconn
```

## Troubleshooting Flowchart

When something goes wrong, follow this decision tree:

```
Start: "docker compose up postgres redis lxp-uniconn" fails
                            │
                            ▼
              ┌─────────────────────────┐
              │ Run: expresso-kit       │
              │ validate --cross-validate│
              └────────────┬────────────┘
                           │
           ┌───────────────┴───────────────┐
           ▼                               ▼
    ┌──────────────┐                ┌─────────────────┐
    │ Validation   │                │ Validation      │
    │ PASSED ✓     │                │ FAILED ✗        │
    └──────┬───────┘                └────────┬────────┘
           │                                 │
           ▼                                 ▼
    ┌──────────────┐                ┌─────────────────┐
    │ Check Docker │                │ Fix errors shown│
    │ daemon running│               │ in output       │
    └──────┬───────┘                └────────┬────────┘
           │                                 │
    ┌──────┴──────┐                         │
    ▼             ▼                         │
 Running?     Not Running?                  │
    │             │                         │
    ▼             ▼                         │
 Continue    Start Docker ←─────────────────┘
    │
    ▼
┌────────────────────────────┐
│ Check: docker compose logs │
│ What error message?        │
└────────────┬───────────────┘
             │
    ┌────────┼────────┬───────────────┐
    ▼        ▼        ▼               ▼
"401/403" "Cannot  "Connection    "Port already
JFrog     connect   refused"      in use"
Auth      to Redis"
    │        │        │               │
    ▼        ▼        ▼               ▼
docker   Check     Wait for       Find process:
login    Redis is  containers     lsof -i :PORT
JFrog    running   to be ready    (Linux/macOS)
                                  netstat -ano
                                  (Windows)
```

### Quick Diagnostic Commands

```bash
# 1. Is Docker running?
docker info > /dev/null 2>&1 && echo "✓ Docker OK" || echo "✗ Start Docker"

# 2. Are containers healthy?
docker compose ps

# 3. What's in the logs?
docker compose logs --tail=50 lxp-uniconn

# 4. Is Postgres ready?
docker compose exec postgres pg_isready

# 5. Is Redis responding?
docker compose exec redis redis-cli ping

# 6. Can Uniconn reach the network?
docker compose exec lxp-uniconn ping -c 1 postgres
```

## Quick Reference Commands

| Task | Command |
|------|---------|
| **Validation** | |
| Validate everything | `expresso-kit validate --cross-validate` |
| Check env files only | `expresso-kit validate --env-only` |
| Check compose only | `expresso-kit validate --compose-only` |
| List services | `expresso-kit list-services --with-env` |
| Check system deps | `expresso-kit check-deps --strict` |
| **Docker Compose** | |
| Start stack | `docker compose up postgres redis lxp-uniconn` |
| Start detached | `docker compose up -d postgres redis lxp-uniconn` |
| View logs | `docker compose logs -f lxp-uniconn` |
| Stop stack | `docker compose down` |
| Stop + remove volumes | `docker compose down -v` |
| Rebuild | `docker compose build --no-cache lxp-uniconn` |
| **Database** | |
| Reset database | `docker compose down -v postgres && docker compose up -d postgres` |
| Enter psql | `docker compose exec postgres psql -U postgres` |
| Load dump | `./load-uniconn-compressed-db.sh dump.sql.gz` |
| **Debugging** | |
| Check health | `curl http://localhost:3000/health` |
| Redis ping | `docker compose exec redis redis-cli ping` |
| Postgres ready | `docker compose exec postgres pg_isready` |

## Automation Scripts

Save time with these ready-to-use scripts.

### Bash/Zsh Setup Script

```bash
#!/usr/bin/env bash
# Save as: setup-uniconn.sh

set -e  # Exit on error

PROJECT_DIR="${1:-.}"
cd "$PROJECT_DIR"

echo "🔍 Validating environment..."
if ! expresso-kit validate --cross-validate; then
    echo "❌ Validation failed. Fix issues above before continuing."
    exit 1
fi

echo "📦 Checking dependencies..."
if ! expresso-kit check-deps --strict; then
    echo "❌ Missing dependencies. Install them first."
    exit 1
fi

echo "🐘 Starting PostgreSQL..."
docker compose up -d postgres

echo "⏳ Waiting for PostgreSQL to be healthy..."
until docker compose exec postgres pg_isready; do
    sleep 2
done

echo "📦 Starting Redis..."
docker compose up -d redis

echo "🦀 Starting Uniconn..."
docker compose up lxp-uniconn --abort-on-container-exit
```

### Fish Shell Setup Script

```fish
#!/usr/bin/env fish
# Save as: setup-uniconn.fish

set project_dir $argv[1]
test -z "$project_dir"; and set project_dir "."
cd $project_dir

echo "🔍 Validating environment..."
expresso-kit validate --cross-validate
if test $status -ne 0
    echo "❌ Validation failed. Fix issues above before continuing."
    exit 1
end

echo "📦 Checking dependencies..."
expresso-kit check-deps --strict
if test $status -ne 0
    echo "❌ Missing dependencies. Install them first."
    exit 1
end

echo "🐘 Starting PostgreSQL..."
docker compose up -d postgres

echo "⏳ Waiting for PostgreSQL..."
while not docker compose exec postgres pg_isready 2>/dev/null
    sleep 2
end

echo "📦 Starting Redis..."
docker compose up -d redis

echo "🦀 Starting Uniconn..."
docker compose up lxp-uniconn --abort-on-container-exit
```

### PowerShell Setup Script (Windows)

```powershell
# Save as: setup-uniconn.ps1

param(
    [string]$ProjectDir = "."
)

Set-Location $ProjectDir
$ErrorActionPreference = "Stop"

Write-Host "🔍 Validating environment..." -ForegroundColor Cyan
expresso-kit validate --cross-validate
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Validation failed. Fix issues above before continuing." -ForegroundColor Red
    exit 1
}

Write-Host "📦 Checking dependencies..." -ForegroundColor Cyan
expresso-kit check-deps --strict
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Missing dependencies. Install them first." -ForegroundColor Red
    exit 1
}

Write-Host "🐘 Starting PostgreSQL..." -ForegroundColor Green
docker compose up -d postgres

Write-Host "⏳ Waiting for PostgreSQL to be healthy..." -ForegroundColor Yellow
do {
    Start-Sleep -Seconds 2
    $result = docker compose exec postgres pg_isready 2>$null
} while ($LASTEXITCODE -ne 0)

Write-Host "📦 Starting Redis..." -ForegroundColor Green
docker compose up -d redis

Write-Host "🦀 Starting Uniconn..." -ForegroundColor Green
docker compose up lxp-uniconn --abort-on-container-exit
```

### Make Scripts Executable

```bash
# Linux/macOS
chmod +x setup-uniconn.sh
chmod +x setup-uniconn.fish

# Run
./setup-uniconn.sh projects/lxp-docker-compose

# Windows PowerShell
.\setup-uniconn.ps1 -ProjectDir "projects\lxp-docker-compose"
```

## Need More Help?

```bash
# Full expresso-kit documentation
expresso-kit --help
expresso-kit validate --help
expresso-kit list-services --help

# Discover all projects in workspace
expresso-kit discover --validate --max-depth 3
```

## 8. Using TUI Mode

The TUI (Terminal User Interface) provides an interactive dashboard for managing repositories, validating environments, and generating workflows—all without memorizing CLI commands.

### Starting the TUI

```bash
# Simply run expresso-kit without arguments
expresso-kit

# Or explicitly with --tui flag
expresso-kit --tui
```

### TUI Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Expresso-Kit Dashboard                             │
├──────────────┬──────────────┬──────────────┬──────────────┬─────────────────┤
│ Repositories │ Environment  │  Workflows   │     Logs     │     Stats       │
├──────────────┴──────────────┴──────────────┴──────────────┴─────────────────┤
│  Select │ Cloned │ Repository        │ Progress     │ Updated  │ Issues    │
│─────────┼────────┼───────────────────┼──────────────┼──────────┼───────────│
│   🔖    │   🟢   │ lxp-docker-compose│ ████████ 100%│ 14:32:01 │    ✓      │
│         │   🟢   │ lxp-uniconn       │ ████████ 100%│ 14:32:05 │    ✓      │
│         │   📡   │ lxp-gibraltar     │ ████░░░░  45%│ 14:32:10 │    —      │
├─────────────────────────────────────────────────────────────────────────────┤
│ [Tab] Switch tabs  [↑↓] Navigate  [Space] Select  [?] Help  [q] Quit        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### TUI Workflow for LXP Setup

#### Step 8.1: Add the LXP Docker Compose Repository

1. Launch TUI: `expresso-kit`
2. Press `a` to add a repository
3. Paste the repository path: `./projects/lxp-docker-compose`
4. Press `Enter` to confirm

#### Step 8.2: Validate Environment (Environment Tab)

1. Press `Tab` to switch to **Environment** tab
2. View all detected `.env` and `.env.sample` files
3. Check for:
   - ✓ Green = properly configured
   - ⚠️ Yellow = warnings (empty values)
   - ❗ Red = errors (missing required vars)

#### Step 8.3: Check Services (Repositories Tab)

1. Press `Tab` to return to **Repositories** tab
2. Select `lxp-docker-compose` with arrow keys
3. Press `Enter` to expand and see:
   - Docker Compose services
   - Missing environment variables
   - Service health status

#### Step 8.4: Validate Selected Repository

1. Select the repository with `↑`/`↓` keys
2. Press `v` to validate
3. Watch the **Progress** column update
4. Check **Issues** column for results:
   - `✓` = all validations passed
   - `2 ⚠` = 2 warnings found

#### Step 8.5: Generate Workflow (Workflows Tab)

1. Press `Tab` twice to reach **Workflows** tab
2. Press `w` to open the workflow generator
3. Select project type (auto-detected as Rust/Docker Compose)
4. Press `Enter` to generate
5. Press `Escape` to close preview

#### Step 8.6: View Logs

1. Press `Tab` to reach **Logs** tab
2. Review all operations performed
3. Look for any error messages

### TUI Keyboard Shortcuts

| Key | Action | Context |
|-----|--------|---------|
| `Tab` | Next tab | Global |
| `Shift+Tab` | Previous tab | Global |
| `↑` / `↓` | Navigate list | All tabs |
| `Enter` | Select/Expand | All tabs |
| `Space` | Toggle selection | Repositories |
| `a` | Add repository | Repositories |
| `d` | Delete selected | Repositories |
| `r` | Refresh/Re-validate | Repositories |
| `v` | Validate selected | Repositories |
| `c` | Clone repository | Repositories |
| `w` | Open workflow generator | Workflows |
| `s` | Save configuration | Global |
| `?` | Show help | Global |
| `q` / `Esc` | Quit/Close popup | Global |

### TUI Status Icons

| Icon | Meaning |
|------|---------|
| 🟢 | Ready - all validations passed |
| 📡 | Cloning in progress |
| 🔍 | Validating |
| ⚠️ | Warning - non-critical issues |
| ❗ | Error - critical issues |
| `-` | Pending - not yet processed |
| 🔖 | Selected for batch operations |

<p align="center">
  Guide made with ☕ <b>expresso-kit</b>
</p>
