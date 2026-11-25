-- Transactional outbox for the Postgres -> OpenSearch read-side sync (CQRS).
-- A command handler inserts into the domain table AND this table in the same
-- transaction; the `worker` binary's outbox dispatcher drains it afterwards.
CREATE TABLE search_outbox (
    id             BIGSERIAL PRIMARY KEY,
    aggregate_type VARCHAR(128) NOT NULL,
    aggregate_id   UUID NOT NULL,
    event_type     VARCHAR(32) NOT NULL CHECK (event_type IN ('upsert', 'delete')),
    payload        JSONB NOT NULL,
    attempts       SMALLINT NOT NULL DEFAULT 0,
    last_error     TEXT,
    failed         BOOLEAN NOT NULL DEFAULT FALSE,
    dispatched_at  TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_search_outbox_pending
    ON search_outbox(created_at)
    WHERE dispatched_at IS NULL AND NOT failed;

-- Outbox for push notifications; the worker's notification sender drains it,
-- honoring notification_preferences and push_subscriptions.
CREATE TABLE notification_outbox (
    id                BIGSERIAL PRIMARY KEY,
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type VARCHAR(64) NOT NULL,
    title             TEXT NOT NULL,
    body              TEXT NOT NULL,
    data              JSONB NOT NULL DEFAULT '{}'::jsonb,
    attempts          SMALLINT NOT NULL DEFAULT 0,
    last_error        TEXT,
    failed            BOOLEAN NOT NULL DEFAULT FALSE,
    sent_at           TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_notification_outbox_pending
    ON notification_outbox(created_at)
    WHERE sent_at IS NULL AND NOT failed;
