-- Add migration script here
CREATE TABLE missions(
    id uuid NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    commands JSON NOT NULL,
    created_at timestamptz NOT NULL
)
