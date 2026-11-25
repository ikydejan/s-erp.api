CREATE TABLE roles (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(64) NOT NULL UNIQUE,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Permission ladder per (role, resource): view=1, create=2, edit=3, delete=4.
-- A role holding level N implicitly has every permission below N on that resource,
-- so "delete" alone is enough to view/create/edit/delete that resource.
CREATE TABLE role_permissions (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role_id   UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    resource  VARCHAR(128) NOT NULL,
    level     SMALLINT NOT NULL CHECK (level BETWEEN 1 AND 4),
    UNIQUE (role_id, resource)
);

CREATE TABLE user_roles (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id    UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, role_id)
);

INSERT INTO roles (name, description) VALUES
    ('admin', 'Full access to every resource'),
    ('staff', 'Baseline operational role, granted per-resource as needed');

INSERT INTO role_permissions (role_id, resource, level)
SELECT id, 'items', 4 FROM roles WHERE name = 'admin';
