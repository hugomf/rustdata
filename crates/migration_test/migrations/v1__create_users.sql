-- @migration V1
-- @description Create users table
-- @col id UUID
-- @col username VARCHAR(255)
-- @col email VARCHAR(255)
-- @col created_at TIMESTAMPTZ
-- @col active BOOLEAN
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    active BOOLEAN NOT NULL DEFAULT TRUE
);
