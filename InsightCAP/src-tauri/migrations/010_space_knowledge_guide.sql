-- Space 知識指南層
-- 每個 Space 有一份 AI 持續維護的結構化知識指南

ALTER TABLE spaces ADD COLUMN knowledge_guide_content TEXT NOT NULL DEFAULT '';
ALTER TABLE spaces ADD COLUMN knowledge_guide_updated_at TEXT NOT NULL DEFAULT '';
