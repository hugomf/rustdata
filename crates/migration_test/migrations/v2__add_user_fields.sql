-- @migration V2
-- @description Add bio and is_premium to users
-- @col bio TEXT
-- @col is_premium BOOLEAN
ALTER TABLE users ADD COLUMN bio TEXT DEFAULT '';
ALTER TABLE users ADD COLUMN is_premium BOOLEAN DEFAULT FALSE;
