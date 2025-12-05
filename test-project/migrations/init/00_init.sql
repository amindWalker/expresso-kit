-- =============================================================================
-- Database Initialization Script
-- =============================================================================
-- This script runs when PostgreSQL container starts for the first time.
-- It creates necessary extensions and initial setup.
-- =============================================================================

-- Enable useful extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Create initial schema (if not using public)
-- CREATE SCHEMA IF NOT EXISTS app;

-- Grant permissions
-- GRANT ALL ON SCHEMA app TO postgres;
