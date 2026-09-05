-- Analytical schema for the data warehouse database
CREATE TABLE IF NOT EXISTS fact_events (
    id              BIGSERIAL PRIMARY KEY,
    event_time      TIMESTAMPTZ NOT NULL,
    account_id      BIGINT NOT NULL,
    event_type      TEXT NOT NULL,
    amount          NUMERIC(12, 2),
    region          TEXT NOT NULL,
    payload         JSONB NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS dim_accounts (
    account_id      BIGINT PRIMARY KEY,
    segment         TEXT NOT NULL,
    country         TEXT NOT NULL,
    signup_date     DATE NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_fact_events_event_time ON fact_events(event_time);
CREATE INDEX IF NOT EXISTS idx_fact_events_account_id ON fact_events(account_id);
CREATE INDEX IF NOT EXISTS idx_fact_events_event_type ON fact_events(event_type);
CREATE INDEX IF NOT EXISTS idx_fact_events_region ON fact_events(region);

INSERT INTO dim_accounts (account_id, segment, country, signup_date)
SELECT
    g,
    CASE (g % 4)
        WHEN 0 THEN 'enterprise'
        WHEN 1 THEN 'pro'
        WHEN 2 THEN 'standard'
        ELSE 'free'
    END,
    CASE (g % 5)
        WHEN 0 THEN 'US'
        WHEN 1 THEN 'EU'
        WHEN 2 THEN 'APAC'
        WHEN 3 THEN 'LATAM'
        ELSE 'MEA'
    END,
    (DATE '2020-01-01' + (g % 2000))::date
FROM generate_series(1, 100000) AS g
ON CONFLICT DO NOTHING;

INSERT INTO fact_events (event_time, account_id, event_type, amount, region, payload)
SELECT
    now() - (g || ' minutes')::interval,
    (random() * 99999 + 1)::bigint,
    CASE (g % 6)
        WHEN 0 THEN 'purchase'
        WHEN 1 THEN 'refund'
        WHEN 2 THEN 'login'
        WHEN 3 THEN 'page_view'
        WHEN 4 THEN 'subscription'
        ELSE 'support_ticket'
    END,
    (random() * 1000)::numeric(12, 2),
    CASE (g % 5)
        WHEN 0 THEN 'US'
        WHEN 1 THEN 'EU'
        WHEN 2 THEN 'APAC'
        WHEN 3 THEN 'LATAM'
        ELSE 'MEA'
    END,
    jsonb_build_object('source', 'seed', 'seq', g)
FROM generate_series(1, 500000) AS g;
