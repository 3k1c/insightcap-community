-- Decision 層（ADR-029）
-- 靜默記錄框架外變數的決策，事後回顧決策品質

CREATE TABLE IF NOT EXISTS decisions (
  id              TEXT PRIMARY KEY,
  project_id      TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  variable_desc   TEXT NOT NULL,
  options         TEXT NOT NULL DEFAULT '[]',
  chosen_option   TEXT NOT NULL DEFAULT '',
  outcome_source  TEXT,
  outcome_rating  TEXT,
  outcome_note    TEXT,
  status          TEXT DEFAULT 'pending',
  trigger_at      TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decisions_project ON decisions(project_id);
CREATE INDEX IF NOT EXISTS idx_decisions_status ON decisions(status);
CREATE INDEX IF NOT EXISTS idx_decisions_trigger ON decisions(trigger_at);
