-- 編譯後知識表（Compiled Knowledge）
-- 由 DeepSynthesisEngine 背景生成，RAG 查詢時直接讀取
-- 每個 space 最多保留一份最新的 compiled knowledge

CREATE TABLE IF NOT EXISTS compiled_knowledge (
  id           TEXT PRIMARY KEY,
  space_id     TEXT REFERENCES spaces(id) ON DELETE CASCADE,
  content      TEXT NOT NULL,
  source_chunk_count INTEGER DEFAULT 0,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

-- space_id 可為 NULL（全域知識），每個 space 最多一份
CREATE UNIQUE INDEX IF NOT EXISTS idx_compiled_knowledge_space
  ON compiled_knowledge(COALESCE(space_id, '__global__'));
