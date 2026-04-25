-- Add is_user_managed column to spaces table
ALTER TABLE spaces ADD COLUMN is_user_managed INTEGER DEFAULT 0;
