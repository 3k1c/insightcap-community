-- Smart Reminder System
-- 從對話自動提取日期/事件，建立智慧提醒

CREATE TABLE IF NOT EXISTS reminders (
  id               TEXT PRIMARY KEY,
  conversation_id  TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  memory_chunk_id  TEXT REFERENCES memory_chunks(id) ON DELETE SET NULL,
  space_id         TEXT REFERENCES spaces(id) ON DELETE SET NULL,
  project_id       TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title            TEXT NOT NULL,
  description      TEXT DEFAULT '',
  event_type       TEXT NOT NULL DEFAULT 'event',
  -- meeting | deliverable | event | appointment
  date_status      TEXT NOT NULL DEFAULT 'confirmed',
  -- confirmed | time_inferred | range | month_only
  event_date       TEXT,           -- YYYY-MM-DD（best-guess 精確日期）
  event_date_end   TEXT,           -- range 類型的結束日期
  event_time       TEXT,           -- HH:MM（有具體時間時）
  confidence       REAL DEFAULT 1.0,
  status           TEXT NOT NULL DEFAULT 'active',
  -- active | completed | dismissed | expired
  pending_confirm  INTEGER DEFAULT 0,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reminders_status ON reminders(status);
CREATE INDEX IF NOT EXISTS idx_reminders_event_date ON reminders(event_date);
CREATE INDEX IF NOT EXISTS idx_reminders_conversation ON reminders(conversation_id);

CREATE TABLE IF NOT EXISTS reminder_notifications (
  id               TEXT PRIMARY KEY,
  reminder_id      TEXT NOT NULL REFERENCES reminders(id) ON DELETE CASCADE,
  intent           TEXT NOT NULL,
  -- start | midcheck | urgent | final | prepare | imminent | now | confirm_date
  scheduled_at     TEXT NOT NULL,  -- ISO 8601，到期時發送
  sent_at          TEXT,           -- NULL = 尚未發送
  channel          TEXT DEFAULT 'desktop',
  -- desktop | telegram | both
  user_action      TEXT,           -- complete | snooze | dismiss | NULL
  created_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reminder_notif_pending
  ON reminder_notifications(scheduled_at) WHERE sent_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_reminder_notif_reminder
  ON reminder_notifications(reminder_id);
