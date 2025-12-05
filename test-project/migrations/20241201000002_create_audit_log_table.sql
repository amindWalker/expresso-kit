-- =============================================================================
-- Migration: Create Audit Log Table
-- =============================================================================
-- Creates an audit log table for tracking important events
-- =============================================================================

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    entity_type VARCHAR(100),
    entity_id UUID,
    old_values JSONB,
    new_values JSONB,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index on user_id for user activity queries
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);

-- Create index on action for filtering by action type
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);

-- Create index on entity for entity-specific queries
CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs(entity_type, entity_id);

-- Create index on created_at for time-based queries
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);

-- Create composite index for common query patterns
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_action_time
    ON audit_logs(user_id, action, created_at DESC);
