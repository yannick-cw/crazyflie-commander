-- Add migration script here
CREATE TABLE flights
(
    id        uuid        NOT NULL PRIMARY KEY,
    name      TEXT        NOT NULL UNIQUE,
    date      timestamptz NOT NULL,
    telemetry JSON        NOT NULL,
    mission   TEXT references missions (name)
)
