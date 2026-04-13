-- Add is_pinned and is_locked to conversations
ALTER TABLE conversations ADD COLUMN is_pinned INTEGER DEFAULT 0;
ALTER TABLE conversations ADD COLUMN is_locked INTEGER DEFAULT 0;
