-- @migration V1
-- @description Create users table
CREATE TABLE users (
    id          UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    username    VARCHAR(255) NOT NULL UNIQUE,
    email       VARCHAR(255),
    age         INTEGER,
    status      VARCHAR(50)  NOT NULL DEFAULT 'active',
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
