-- 006: Repository Timeline 支援
-- 為 sources 新增分類欄位與本地文件路徑
-- 為 captures 新增用戶編輯標記

-- sources: 檔案分類（editor_doc | captured）
ALTER TABLE sources ADD COLUMN source_category TEXT NOT NULL DEFAULT 'captured';

-- sources: 媒體類型（text | url | image | video | pdf）
ALTER TABLE sources ADD COLUMN media_type TEXT NOT NULL DEFAULT 'text';

-- sources: 編輯器文件本地路徑（僅 editor_doc 類使用）
ALTER TABLE sources ADD COLUMN local_doc_path TEXT;

-- captures: 用戶是否手動編輯過此 chunk
ALTER TABLE captures ADD COLUMN is_user_edited INTEGER NOT NULL DEFAULT 0;

-- 索引：按分類 + 時間排序（Timeline 查詢用）
CREATE INDEX IF NOT EXISTS idx_sources_category_captured
  ON sources(source_category, captured_at DESC);

-- 索引：按媒體類型篩選
CREATE INDEX IF NOT EXISTS idx_sources_media_type
  ON sources(media_type);
