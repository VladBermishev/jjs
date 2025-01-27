CREATE DOMAIN unsigned_integer AS INTEGER
    CHECK (VALUE >= 0);

-- Users table

CREATE SEQUENCE user_id_seq START WITH 0 MINVALUE 0;

CREATE TABLE users
(
    id            UUID UNIQUE PRIMARY KEY NOT NULL,
    username      VARCHAR(100) UNIQUE     NOT NULL,
    password_hash CHAR(128), -- SHA3-512, in hex encoding
    groups        TEXT[]                  NOT NULL
);

INSERT INTO users
values ('04eb5beb-bf14-459c-bcf1-57eca87a0055'::uuid,
        'Global/Root',
        NULL,
        '{}'),
       ('56ff846e-81bd-451b-aeea-90afc192bd77'::uuid,
        'Global/Guest',
        NULL,
        '{}');

-- Runs

CREATE SEQUENCE run_id_seq START WITH 0 MINVALUE 0;

CREATE TABLE runs
(
    id           unsigned_integer DEFAULT nextval('run_id_seq') PRIMARY KEY NOT NULL,
    toolchain_id VARCHAR(100)                                               NOT NULL,
    status_code  VARCHAR(100)                                               NOT NULL,
    status_kind  VARCHAR(100)                                                NOT NULL,
    problem_id   VARCHAR(100)                                               NOT NULL,
    score        INTEGER                                                                  NOT NULL,
    rejudge_id   unsigned_integer                                            NOT NULL,
    user_id UUID REFERENCES users(id) NOT NULL
);

CREATE UNIQUE INDEX runs_id_unique_index ON runs (id);

CREATE SEQUENCE inv_req_id_seq START WITH 0 MINVALUE 0;

-- Invocation requests

CREATE table invocation_requests
(
    id              unsigned_integer DEFAULT nextval('inv_req_id_seq') UNIQUE PRIMARY KEY NOT NULL,
    run_id          unsigned_integer REFERENCES runs (id)                                 NOT NULL,
    invoke_revision unsigned_integer                                                      NOT NULL
);