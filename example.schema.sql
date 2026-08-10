-- Demo schema for the example/ queries

BEGIN;

DROP TABLE IF EXISTS audit, orders, users;

CREATE TABLE users (
    id         bigserial PRIMARY KEY,
    email      text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE orders (
    id         bigserial PRIMARY KEY,
    user_id    bigint NOT NULL REFERENCES users (id),
    status     text NOT NULL,
    total      numeric(10, 2) NOT NULL,
    created_at timestamptz NOT NULL
);

CREATE TABLE audit (
    id         bigserial PRIMARY KEY,
    table_name text NOT NULL,
    action     text NOT NULL,
    "column :'still_not_one'" text,
    created_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO users (id, email, created_at) VALUES
    (42, 'ada@example.com',   now() - interval '400 days'),
    (43, 'linus@example.com', now() - interval '10 days'),
    (44, 'grace@example.com', now() - interval '3 days');
SELECT setval('users_id_seq', 44);

INSERT INTO orders (user_id, status, total, created_at) VALUES
    (42, 'in progress', 42.00,  '2026-01-05'),
    (42, 'shipped',     13.50,  '2026-01-12'),
    (43, 'in progress', 128.90, '2026-01-20'),
    (43, 'cancelled',   7.00,   '2026-02-03'),
    (44, 'in progress', 64.25,  '2026-03-11');

INSERT INTO audit (table_name, action, "column :'still_not_one'") VALUES
    ('orders', 'insert', 'seeded'),
    ('orders', 'update', 'seeded'),
    ('users',  'insert', 'seeded'),
    ('users',  'delete', 'seeded');

COMMIT;
