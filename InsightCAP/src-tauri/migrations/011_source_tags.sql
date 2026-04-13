-- 為 sources 表新增 tags 欄位（Source 層級標籤）
ALTER TABLE sources ADD COLUMN tags TEXT DEFAULT '[]';
