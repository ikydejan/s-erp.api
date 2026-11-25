CREATE TABLE delegations (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    delegator_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    delegate_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- NULL = delegate inherits every permission of the delegator; otherwise a
    -- JSON array of resource names the delegation is scoped to, e.g. ["items"].
    scope        JSONB,
    reason       TEXT,
    start_at     TIMESTAMPTZ NOT NULL,
    end_at       TIMESTAMPTZ NOT NULL,
    status       VARCHAR(16) NOT NULL DEFAULT 'active'
                 CHECK (status IN ('active', 'expired', 'revoked')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (end_at > start_at),
    CHECK (delegator_id <> delegate_id)
);

CREATE INDEX idx_delegations_delegate_active
    ON delegations(delegate_id, status, start_at, end_at);
CREATE INDEX idx_delegations_delegator ON delegations(delegator_id);
