-- Phase 5: Enterprise 外部知識庫掛載
CREATE TABLE IF NOT EXISTS external_knowledge_bases (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  uri            TEXT NOT NULL,     -- 指向外部 SQLite 檔案或服務的位置
  status         TEXT DEFAULT 'active',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
