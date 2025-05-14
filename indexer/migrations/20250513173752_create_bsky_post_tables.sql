CREATE SCHEMA IF NOT EXISTS eyewall;

CREATE TABLE eyewall.bsky_posts (
    author_did TEXT NOT NULL,
    rkey TEXT NOT NULL,
    created_at TIMESTAMP,
    PRIMARY KEY (author_did, rkey)
);

CREATE TABLE eyewall.bsky_post_replies (
    author_did TEXT NOT NULL,
    rkey TEXT NOT NULL,
    parent_author_did TEXT NOT NULL,
    parent_rkey TEXT NOT NULL,
    PRIMARY KEY (author_did, rkey),
    FOREIGN KEY (author_did, rkey) REFERENCES bsky_posts(author_did, rkey),
    FOREIGN KEY (parent_author_did, parent_rkey) REFERENCES bsky_posts(author_did, rkey)
);

CREATE TABLE eyewall.bsky_post_quotes (
    author_did TEXT NOT NULL,
    rkey TEXT NOT NULL,
    parent_author_did TEXT NOT NULL,
    parent_rkey TEXT NOT NULL,
    PRIMARY KEY (author_did, rkey),
    FOREIGN KEY (author_did, rkey) REFERENCES bsky_posts(author_did, rkey),
    FOREIGN KEY (parent_author_did, parent_rkey) REFERENCES bsky_posts(author_did, rkey)
);
