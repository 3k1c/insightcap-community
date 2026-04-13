-- Space Wiki 層
-- 每個 Space 有一份 AI 持續維護的結構化知識文件

ALTER TABLE spaces ADD COLUMN wiki_content TEXT NOT NULL DEFAULT '';
ALTER TABLE spaces ADD COLUMN wiki_updated_at TEXT NOT NULL DEFAULT '';
