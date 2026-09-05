-- OLTP schema for the "live" database
CREATE TABLE IF NOT EXISTS accounts (
    id          BIGSERIAL PRIMARY KEY,
    email       TEXT NOT NULL UNIQUE,
    balance     NUMERIC(12, 2) NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS orders (
    id          BIGSERIAL PRIMARY KEY,
    account_id  BIGINT NOT NULL REFERENCES accounts(id),
    amount      NUMERIC(12, 2) NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_orders_account_id ON orders(account_id);
CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at);
CREATE INDEX IF NOT EXISTS idx_accounts_status ON accounts(status);

-- Seed a small baseline so reads have data to hit
INSERT INTO accounts (email, balance)
SELECT
    'user_' || g || '@example.com',
    (random() * 10000)::numeric(12, 2)
FROM generate_series(1, 10000) AS g
ON CONFLICT DO NOTHING;

INSERT INTO orders (account_id, amount, status)
SELECT
    (random() * 9999 + 1)::bigint,
    (random() * 500)::numeric(12, 2),
    CASE WHEN random() < 0.8 THEN 'completed' ELSE 'pending' END
FROM generate_series(1, 50000);
