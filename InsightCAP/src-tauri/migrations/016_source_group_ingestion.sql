CREATE TABLE IF NOT EXISTS source_groups (
  id             TEXT PRIMARY KEY,
  group_type     TEXT NOT NULL DEFAULT 'file',
  canonical_key  TEXT NOT NULL UNIQUE,
  canonical_url  TEXT,
  title          TEXT NOT NULL DEFAULT '',
  content_hash   TEXT,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

ALTER TABLE sources ADD COLUMN source_group_id TEXT REFERENCES source_groups(id) ON DELETE SET NULL;

ALTER TABLE captures ADD COLUMN content_type TEXT NOT NULL DEFAULT 'prose';
ALTER TABLE captures ADD COLUMN knowledge_type TEXT NOT NULL DEFAULT 'data';
ALTER TABLE captures ADD COLUMN chunk_strategy TEXT NOT NULL DEFAULT 'paragraph';
ALTER TABLE captures ADD COLUMN chunk_metadata TEXT DEFAULT '{}';

CREATE INDEX IF NOT EXISTS idx_sources_source_group_id ON sources(source_group_id);
CREATE INDEX IF NOT EXISTS idx_source_groups_canonical_key ON source_groups(canonical_key);
CREATE INDEX IF NOT EXISTS idx_captures_knowledge_type ON captures(knowledge_type);
CREATE INDEX IF NOT EXISTS idx_captures_content_type ON captures(content_type);
