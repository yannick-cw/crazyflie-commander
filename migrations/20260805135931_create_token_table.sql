-- Add migration script here
CREATE TABLE tokens
(
    id           uuid        NOT NULL PRIMARY KEY,
    label        TEXT        NOT NULL UNIQUE,
    token_hash   bytea       NOT NULL UNIQUE,
    created_at   timestamptz NOT NULL,
    revoked_at   timestamptz,
    last_used_at timestamptz
)
