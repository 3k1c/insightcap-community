-- Chunk 關聯關係（反向鏈接）
-- 記錄 capture / memory_chunk 之間的顯式語義關係
-- 供 RAG 查詢時串聯關聯資訊，以及前端引用預覽展示

CREATE TABLE IF NOT EXISTS chunk_relations (
  id          TEXT PRIMARY KEY,
  from_id     TEXT NOT NULL,        -- capture_id 或 memory_chunk_id
  to_id       TEXT NOT NULL,
  from_type   TEXT NOT NULL,        -- 'capture' | 'memory_chunk'
  to_type     TEXT NOT NULL,        -- 'capture' | 'memory_chunk'
  relation    TEXT NOT NULL,        -- 'references' | 'contradicts' | 'extends'
  confidence  REAL NOT NULL DEFAULT 1.0,
  created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunk_relations_from ON chunk_relations(from_id);
CREATE INDEX IF NOT EXISTS idx_chunk_relations_to   ON chunk_relations(to_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_chunk_relations_pair ON chunk_relations(from_id, to_id, relation);
