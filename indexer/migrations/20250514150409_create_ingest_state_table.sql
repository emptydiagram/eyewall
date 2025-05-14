CREATE TABLE eyewall.ingest_state (
    id INTEGER NOT NULL DEFAULT 1,
    bsky_post_cursor TIMESTAMP NOT NULL,
    CONSTRAINT ingest_state_pk PRIMARY KEY (id),
    CONSTRAINT ingest_state_id_chk CHECK (id = 1)
);

INSERT INTO eyewall.ingest_state (bsky_post_cursor) VALUES (to_timestamp(1735711200.000000));