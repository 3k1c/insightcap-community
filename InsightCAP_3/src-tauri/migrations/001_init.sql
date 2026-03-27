-- InsightCAP v2 全新 Schema
-- Phase 1: 基礎建設

-- sources 表（原文件/來源，用戶感知層）
CREATE TABLE IF NOT EXISTS sources (
  id            TEXT PRIMARY KEY,
  type          TEXT NOT NULL,
  -- file | url | image | editor | clipboard | screenshot
  title         TEXT NOT NULL,
  url           TEXT,
  file_path     TEXT,
  thumbnail     TEXT,
  clean_content TEXT NOT NULL DEFAULT '',
  content_hash  TEXT,
  capture_count INTEGER DEFAULT 0,
  use_frequency INTEGER DEFAULT 0,
  captured_at   TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

-- captures 表（chunk 粒度，後台計算單位）
CREATE TABLE IF NOT EXISTS captures (
  id               TEXT PRIMARY KEY,
  source_id        TEXT REFERENCES sources(id) ON DELETE CASCADE,
  space_id         TEXT REFERENCES spaces(id) ON DELETE SET NULL,
  type             TEXT NOT NULL,
  raw_content      TEXT NOT NULL DEFAULT '',
  clean_content    TEXT NOT NULL DEFAULT '',
  image_path       TEXT,
  capture_method   TEXT NOT NULL,
  -- hotkey | import | mobile | editor_export | url
  tags             TEXT DEFAULT '[]',
  chunk_index      INTEGER DEFAULT 0,
  vector_id        INTEGER,
  status           TEXT DEFAULT 'inbox',
  -- inbox | processed | archived | pending_ocr
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);

-- memory_chunks 表（三層記憶物理載體，只從對話產出）
CREATE TABLE IF NOT EXISTS memory_chunks (
  id               TEXT PRIMARY KEY,
  source_id        TEXT REFERENCES sources(id) ON DELETE SET NULL,
  space_id         TEXT REFERENCES spaces(id) ON DELETE SET NULL,
  conversation_id  TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  project_id       TEXT REFERENCES projects(id) ON DELETE SET NULL,
  knowledge_type   TEXT NOT NULL DEFAULT 'data',
  -- data | pattern | log
  content          TEXT NOT NULL,
  tags             TEXT DEFAULT '[]',
  trigger_context  TEXT DEFAULT '',
  -- log 專用，| 分隔，substring match
  confidence       REAL DEFAULT 1.0,
  pending_confirm  INTEGER DEFAULT 0,
  promotion_count  INTEGER DEFAULT 0,
  vector_id        INTEGER,
  placed_by        TEXT DEFAULT 'ai',
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);

-- spaces 表（AI 後台聚類）
CREATE TABLE IF NOT EXISTS spaces (
  id               TEXT PRIMARY KEY,
  name             TEXT NOT NULL,
  description      TEXT DEFAULT '',
  embedding_center BLOB,
  chunk_count      INTEGER DEFAULT 0,
  created_by       TEXT DEFAULT 'ai',
  is_archived      INTEGER DEFAULT 0,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);

-- tags 表（獨立標籤，支援頻率統計）
CREATE TABLE IF NOT EXISTS tags (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL UNIQUE,
  source       TEXT DEFAULT 'ai',
  use_count    INTEGER DEFAULT 0,
  recent_count INTEGER DEFAULT 0,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

-- projects 表
CREATE TABLE IF NOT EXISTS projects (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  default_tags   TEXT DEFAULT '[]',
  is_pinned      INTEGER DEFAULT 0,
  is_archived    INTEGER DEFAULT 0,
  sort_order     INTEGER DEFAULT 0,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

-- conversations 表
CREATE TABLE IF NOT EXISTS conversations (
  id             TEXT PRIMARY KEY,
  project_id     TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title          TEXT DEFAULT '',
  summary        TEXT DEFAULT '',
  tags           TEXT DEFAULT '[]',
  query_scope    TEXT DEFAULT '{}',
  message_count  INTEGER DEFAULT 0,
  is_archived    INTEGER DEFAULT 0,
  sort_order     INTEGER DEFAULT 0,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

-- messages 表
CREATE TABLE IF NOT EXISTS messages (
  id              TEXT PRIMARY KEY,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
  role            TEXT NOT NULL,
  -- user | assistant | system
  content         TEXT NOT NULL,
  metadata        TEXT DEFAULT '{}',
  created_at      TEXT NOT NULL
);

-- inbox 表（擷取佇列）
CREATE TABLE IF NOT EXISTS inbox (
  id             TEXT PRIMARY KEY,
  content        TEXT NOT NULL DEFAULT '',
  content_type   TEXT NOT NULL DEFAULT 'text',
  -- text | image | file | url
  source_exe     TEXT DEFAULT '',
  window_title   TEXT DEFAULT '',
  session_id     TEXT DEFAULT '',
  image_path     TEXT,
  file_path      TEXT,
  status         TEXT DEFAULT 'pending',
  -- pending | processing | processed | failed
  captured_at    TEXT NOT NULL
);

-- settings 表
CREATE TABLE IF NOT EXISTS settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL
);

-- conversation_summary_queue 表
CREATE TABLE IF NOT EXISTS conversation_summary_queue (
  id              TEXT PRIMARY KEY,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
  trigger_type    TEXT NOT NULL,
  -- manual | minimize_or_close | switch
  status          TEXT DEFAULT 'pending',
  -- pending | processing | done | failed
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_captures_source_id ON captures(source_id);
CREATE INDEX IF NOT EXISTS idx_captures_space_id ON captures(space_id);
CREATE INDEX IF NOT EXISTS idx_captures_status ON captures(status);
CREATE INDEX IF NOT EXISTS idx_memory_chunks_knowledge_type ON memory_chunks(knowledge_type);
CREATE INDEX IF NOT EXISTS idx_memory_chunks_pending_confirm ON memory_chunks(pending_confirm);
CREATE INDEX IF NOT EXISTS idx_memory_chunks_conversation_id ON memory_chunks(conversation_id);
CREATE INDEX IF NOT EXISTS idx_messages_conversation_id ON messages(conversation_id);
CREATE INDEX IF NOT EXISTS idx_inbox_status ON inbox(status);
CREATE INDEX IF NOT EXISTS idx_sources_captured_at ON sources(captured_at);
CREATE INDEX IF NOT EXISTS idx_conversations_project_id ON conversations(project_id);
