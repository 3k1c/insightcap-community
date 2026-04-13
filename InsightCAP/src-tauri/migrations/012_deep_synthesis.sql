-- 深度合成引擎追蹤欄位
-- memory_chunks 新增 last_synthesized_at 記錄上次被合成處理的時間
-- 防止短時間內重複對同一 chunk 合成

ALTER TABLE memory_chunks ADD COLUMN last_synthesized_at TEXT DEFAULT NULL;

CREATE INDEX IF NOT EXISTS idx_memory_chunks_last_synthesized ON memory_chunks(last_synthesized_at);
