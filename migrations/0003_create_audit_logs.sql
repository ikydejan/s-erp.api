CREATE TABLE audit_logs (
    id                  BIGSERIAL PRIMARY KEY,
    actor_user_id       UUID REFERENCES users(id) ON DELETE SET NULL,
    acted_on_behalf_of  UUID REFERENCES users(id) ON DELETE SET NULL,
    action              VARCHAR(64) NOT NULL,
    resource_type       VARCHAR(128),
    resource_id         VARCHAR(128),
    before_state        JSONB,
    after_state         JSONB,
    ip_address          INET,
    user_agent          TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_logs_actor ON audit_logs(actor_user_id, occurred_at DESC);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action, occurred_at DESC);

-- ISO/IEC 27001:2022 A.8.15 (logging): audit records must be tamper-resistant.
-- In production, the application should connect through a restricted role that
-- can only INSERT and SELECT here, never UPDATE or DELETE. We create that role
-- below; point DATABASE_URL at it instead of the table owner in production.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'erp_app') THEN
        CREATE ROLE erp_app LOGIN PASSWORD 'change-me-in-production';
    END IF;
END $$;

GRANT CONNECT ON DATABASE erp TO erp_app;
GRANT USAGE ON SCHEMA public TO erp_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO erp_app;
GRANT SELECT, UPDATE, USAGE ON ALL SEQUENCES IN SCHEMA public TO erp_app;

-- Narrow audit_logs specifically: revoke write/delete, leave only append + read.
REVOKE UPDATE, DELETE ON audit_logs FROM erp_app;
GRANT SELECT, INSERT ON audit_logs TO erp_app;
GRANT USAGE ON SEQUENCE audit_logs_id_seq TO erp_app;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO erp_app;
