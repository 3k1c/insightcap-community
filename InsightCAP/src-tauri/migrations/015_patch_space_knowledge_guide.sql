-- 確保 spaces 表包含了 knowledge_guide_content
ALTER TABLE spaces ADD COLUMN knowledge_guide_content TEXT NOT NULL DEFAULT '';
ALTER TABLE spaces ADD COLUMN knowledge_guide_updated_at TEXT NOT NULL DEFAULT '';
