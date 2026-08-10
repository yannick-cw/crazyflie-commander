-- Add migration script here
CREATE TYPE header_pair AS
(
    name  text,
    value bytea
);

CREATE TABLE idempotency
(
    label                text        not null REFERENCES tokens (label),
    idempotency_key      text        not null,
    response_status_code smallint,
    response_headers     header_pair[],
    response_body        bytea,
    created_at           timestamptz not null,
    PRIMARY KEY (label, idempotency_key)
)