# S-ERP API

Rust ERP backend: Axum + PostgreSQL (write side) + OpenSearch (read side, CQRS),
JWT auth with TOTP MFA, permission-ladder RBAC, delegation, audit trail, and
Web Push notifications. Built as a Cargo workspace so each concern (auth,
audit, delegation, notifications, CQRS wiring, Postgres/OpenSearch adapters)
is its own crate — new ERP modules copy the `items` module's shape rather
than growing one giant crate.

## Architecture

```
                         ┌─────────────┐        ┌──────────────┐
                HTTP ───▶│  api (bin)  │        │ worker (bin) │
                         │  axum +     │        │  3 background│
                         │  Swagger UI │        │  loops       │
                         └──────┬──────┘        └──────┬───────┘
                                │                       │
          ┌─────────────────────┼───────────────────────┤
          │                     │                       │
   ┌──────▼─────┐      ┌────────▼────────┐     ┌────────▼────────┐
   │ auth       │      │ infra-postgres  │     │ infra-opensearch │
   │ audit      │◀─────│  (write side,   │     │  (read side)     │
   │ delegation │ ports│   source of     │     │                  │
   │ notification│     │   truth)        │     │                  │
   │ cqrs       │      └────────┬────────┘     └────────▲─────────┘
   └──────┬─────┘               │                       │
          │                     ▼                       │
          │              ┌─────────────┐                │
          │              │  PostgreSQL │                │
          │              └──────┬──────┘                │
          │                     │ search_outbox          │
          └─────────────────────┴────────────────────────┘
                  (worker's outbox dispatcher drains it
                   into OpenSearch)
```

- `erp-core` — domain entities, `AppError`, `ApiResponse<T>` envelope. No infra deps.
- `auth` — password policy/hashing (Argon2), JWT, TOTP MFA + backup codes, RBAC permission-ladder helpers. No infra deps.
- `audit` — `AuditLogger` port + `AuditEntry` model. No infra deps.
- `delegation` — `Delegation` domain + `DelegationRepository` port. No infra deps.
- `notification` — push subscription/preference models, `NotificationRepository` port, Web Push sender (VAPID). No DB deps.
- `cqrs` — command/query **ports** + DTOs for the reference "items" module. No infra deps.
- `infra-postgres` — SQLx pool, migrations runner, every Postgres repository (implements the ports above).
- `infra-opensearch` — OpenSearch client + the read-side repository for "items".
- `api` (bin) — Axum HTTP server: routes, JWT/RBAC/delegation middleware, Swagger UI.
- `worker` (bin) — three background loops: outbox dispatcher (Postgres → OpenSearch), notification sender (Web Push), delegation sweeper (auto-expire).

Everything outside `api`/`worker` only depends on `erp-core` and talks to the
outside world through traits (`AuditLogger`, `DelegationRepository`,
`NotificationRepository`, `ItemWriteRepository`, `ItemSearchRepository`,
`SearchIndexer`). `infra-postgres`/`infra-opensearch` are the only crates that
know SQL or OpenSearch exist.

## What's fully implemented end-to-end

- **Auth**: register, login (with account lockout after 5 failed attempts),
  JWT access + rotating refresh tokens, logout.
- **MFA**: TOTP setup/enable/disable, 10 single-use backup codes, a
  short-lived "MFA pending" token that only the `/auth/mfa/verify` endpoint
  accepts (it is rejected everywhere else).
- **Password policy**: min 12 chars + character classes, last-5 reuse check,
  `password_changed_at` tracked (no forced rotation — not recommended by
  current guidance, but the data is there if you want to add it).
- **RBAC**: permission ladder (`view < create < edit < delete`) per
  `(role, resource)`, merged across all of a user's roles, enforced via
  `require_permission()` in each handler (and audit-logged on denial).
- **Audit trail**: every data mutation and every auth/permission event is
  written to `audit_logs`. The table is **structurally append-only** — the
  `erp_app` DB role created by migration `0003` only has `INSERT`/`SELECT` on
  it; point `DATABASE_URL` at that role in production instead of the table
  owner.
- **CQRS reference module ("items")**: `POST /api/v1/items` writes to
  Postgres + an `audit_logs` row + a `search_outbox` row in one transaction;
  the `worker` outbox dispatcher indexes it into OpenSearch;
  `GET /api/v1/items` reads from OpenSearch, `GET /api/v1/items/{id}` reads
  Postgres directly (so a just-created record is visible before the outbox
  catches up). Copy this module's shape for the next one.
- **Delegation**: `POST /api/v1/delegations` (delegate your access for a time
  window, optionally scoped to specific resources), `DELETE .../{id}`
  (revoke), `GET .../mine` (who's delegated to me). `POST /auth/act-as`
  issues a token that carries the delegation; while using it, the caller's
  effective permissions become the delegator's (filtered by scope), and
  every action is audit-logged with `acted_on_behalf_of` set. The `worker`
  sweeper auto-expires past-due delegations and notifies the delegator.
- **Push notifications**: subscribe/unsubscribe endpoints, per-type on/off
  preferences, and a `worker` sender that delivers via Web Push (VAPID) —
  works with the browser closed, as long as the OS push service is running.
  No frontend/service-worker code here by design (API-only repo).
- **Versioning**: all routes under `/api/v1/...`; a future `v2` of one module
  is added under `routes/v1` → `routes/v2` without touching existing files.
- **Consistent responses**: every endpoint returns `ApiResponse<T>`
  (`{ success, data, meta: { request_id, pagination? }, error }`); all
  errors funnel through one `AppError → IntoResponse` impl.
- **Swagger**: `utoipa` annotations on every handler/DTO, served at
  `GET /swagger-ui`, raw spec at `GET /api-docs/openapi.json`.

## What's intentionally stubbed / left for the next module

- Only one business module (`items`) is wired end-to-end as the reference
  pattern — the point of this scaffold is that the next module copies
  `cqrs::{dto,ports}` + `infra-postgres/item_repository.rs` +
  `infra-opensearch/item_repository.rs` + `api/routes/v1/items.rs`.
- No admin UI for assigning roles — migration `0002` seeds `admin` (full
  `items` access) and `staff` (no permissions) roles; new users get `staff`.
  Promote a user with SQL until an admin module exists:
  `INSERT INTO user_roles (user_id, role_id) SELECT '<user-id>', id FROM roles WHERE name = 'admin';`
- MFA secret encryption key (`MFA_ENCRYPTION_KEY`) is a plain env var — real
  production use should pull it from a KMS/secrets manager with rotation;
  the AES-256-GCM encrypt/decrypt code itself is real, not a placeholder.
- The outbox/notification pollers are simple fixed-interval loops (5s), not
  `LISTEN/NOTIFY`-driven — fine at ERP scale, easy to swap later.
- No rate limiting on `/auth/login` beyond the account lockout itself.

## Database access note

SQLx's compile-time query macros (`query!`) need a live database at build
time (or a committed `.sqlx` offline cache) to type-check SQL against the
real schema. Since this environment has no persistent database, every query
uses the runtime-checked `sqlx::query`/`query_as` instead — the trade-off is
SQL typos surface at runtime (in tests/integration, not `cargo check`) rather
than at compile time. If your team prefers compile-time checking, run
`cargo sqlx prepare --workspace` once a database is reachable and commit the
resulting `.sqlx` directory.

## Running locally

```bash
cp .env.example .env
# fill in JWT_SECRET and MFA_ENCRYPTION_KEY (see comments in .env.example
# for how to generate them), leave VAPID_* empty until you run gen_vapid_keys

docker compose up -d postgres opensearch

cargo run -p api      # runs migrations on startup, serves on :8080
cargo run -p worker   # outbox dispatcher + notification sender + delegation sweeper
```

- Swagger UI: http://localhost:8080/swagger-ui
- Health check: http://localhost:8080/api/v1/health
- Generate a VAPID keypair for push notifications:
  `cargo run -p worker --bin gen_vapid_keys` (prints `VAPID_PRIVATE_KEY`/`VAPID_PUBLIC_KEY` to paste into `.env`)

### Verified in this environment

- `cargo build --workspace` — compiles clean (no errors, no warnings besides
  an upstream `sqlx-postgres` future-incompatibility notice unrelated to this
  code).
- All 7 migrations were run against a real local PostgreSQL 16 instance
  (Docker wasn't available in the sandbox that built this, so a local
  `postgresql-16` package was used instead) — schema, seed data, and the
  `erp_app` append-only grant on `audit_logs` were confirmed by inspecting
  the resulting database directly.
- OpenSearch itself could not be started in that sandbox (no Docker daemon,
  and OpenSearch needs a JVM container), so the read-side (`GET /items`) and
  the `worker`'s outbox indexing were verified by code review and by
  confirming the `api`/`worker` binaries reach the OpenSearch connection
  step correctly (they fail there with a plain connection-refused, as
  expected, once Postgres migrations already succeeded). Bring up
  `docker compose up -d` in a normal dev machine to exercise that path.

## Environment variables

See `.env.example` for the full list with comments. The required ones with
no safe default are `DATABASE_URL`, `JWT_SECRET`, and `MFA_ENCRYPTION_KEY`.

## Deploy (Sandbox & Production)

`.github/workflows/_deploy.yml` is a reusable workflow: it builds the `api`
and `worker` images (`Dockerfile`, selected via `--build-arg
BIN_NAME=api|worker`), pushes them to GHCR, then SSHes into a server to pull
and restart them with `docker-compose.deploy.yml`. Two thin workflows call
it with different [GitHub Environments](https://docs.github.com/en/actions/deployment/targeting-different-environments/using-environments-for-deployment):

| Workflow                   | Triggers on push to | GitHub Environment | Image tag  |
|------------------------------|------------------------|-----------------------|--------------|
| `deploy-sandbox.yml`         | `sandbox`                 | `sandbox`                | `sandbox`      |
| `deploy-production.yml`      | `production`              | `production`             | `production`   |

Because each is a separate **GitHub Environment** (repo Settings →
Environments), both can define secrets with the *same names* — `HOST`,
`SSH_USER`, `SSH_KEY`, `DEPLOY_PATH` — with different values, and the
workflow automatically reads the right one depending on which environment
the job runs under. Nothing needs a `SANDBOX_`/`PROD_` prefix.

**Production requires manual approval before it deploys.** This is a GitHub
setting, not something the workflow file can do on its own: go to repo
**Settings → Environments → production → Deployment protection rules →
Required reviewers**, and add at least one reviewer. With that set, the
`deploy` job of `deploy-production.yml` (the part that SSHes into the
server — the image build/push still runs immediately) pauses and waits for
an approval in the Actions run page before it touches the production
server. `sandbox` is left without this rule, so it deploys immediately on
push.

CI never creates or touches secrets on the server — it only pulls images and
runs `docker compose up -d`. Before the first deploy to an environment, set
up its server once:

1. Create the deploy directory, e.g. `/opt/s-erp-api`, owned by `SSH_USER`
   (`sudo mkdir -p /opt/s-erp-api && sudo chown -R <ssh_user>:<ssh_user> /opt/s-erp-api`).
2. Put a real `.env` file in it (copy `.env.example` as a starting point).
   `docker-compose.deploy.yml` does **not** run Postgres as a container —
   both environments use a Postgres that already runs on the host OS, so
   point `DATABASE_URL` at `host.docker.internal` (mapped to the host via
   `extra_hosts` in the compose file) instead of `localhost`, and point
   `OPENSEARCH_URL` at the `opensearch` container, which this compose file
   does run:
   ```
   DATABASE_URL=postgres://<db_user>:<db_password>@host.docker.internal:5432/<db_name>
   OPENSEARCH_URL=http://opensearch:9200
   ```
   and use real values for `JWT_SECRET`, `MFA_ENCRYPTION_KEY`, and `VAPID_*`.

   The host's Postgres also needs to accept that connection, which a
   default install usually doesn't out of the box:
   - `listen_addresses` in `postgresql.conf` must include `*` (or at least
     the Docker bridge address, not just `localhost`).
   - `pg_hba.conf` needs a line allowing the Docker bridge subnet, e.g.
     `host    <db_name>    <db_user>    172.17.0.0/16    scram-sha-256`
     (check `docker network inspect bridge` if your subnet differs).
   - Restart Postgres after changing either file.
3. Make sure Docker + the Compose plugin are installed on the server, and
   that `SSH_USER` can run `docker` (e.g. is in the `docker` group).

For **each** environment (`sandbox` and `production`), under repo
**Settings → Environments → <name> → Add secret**, set:

| Secret            | Purpose                                                      |
|---------------------|------------------------------------------------------------------|
| `HOST`                | That environment's server hostname/IP                            |
| `SSH_USER`            | SSH user used to deploy                                           |
| `SSH_KEY`             | Private key (PEM) authorized for that user                        |
| `DEPLOY_PATH`         | Absolute path on the server holding `.env` (e.g. `/opt/s-erp-api`) |

Optional secret (same "Add secret" screen, not "Variables"): `SSH_PORT`
(defaults to `22`).

No secret is needed for the GHCR push/pull — it uses the workflow's own
`GITHUB_TOKEN`.
