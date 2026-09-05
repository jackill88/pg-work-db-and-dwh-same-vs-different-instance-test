#!/bin/bash
set -euo pipefail

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname postgres <<-EOSQL
    SELECT 'CREATE DATABASE live' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'live')\gexec
    SELECT 'CREATE DATABASE dwh' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'dwh')\gexec
EOSQL

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname live -f /schemas/01-live-schema.sql
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname dwh -f /schemas/02-dwh-schema.sql
