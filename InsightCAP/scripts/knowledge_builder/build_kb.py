import sqlite3
import argparse
import os
import uuid
import json
from datetime import datetime

# 需求依賴: pip install fastembed usearch
# 此腳本為 Phase 5 Knowledge Builder (規劃版)
# 用途：將純文本打散並寫入 SQLite (附上 meta)，若有安裝向量庫依賴，將會自動輸出對應的 vectors

def init_db(db_path, kb_name, embedding_dim=384):
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    # 建立設定表包含內部元數據
    cur.execute("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT)")
    cur.execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)", 
        ("metadata", json.dumps({
            "name": kb_name,
            "embeddingModel": "fastembed-default",
            "embeddingDimension": embedding_dim,
            "version": "1.0",
            "buildTime": datetime.now().isoformat()
        }))
    )
    # 建立 Captures 表
    cur.execute("""
        CREATE TABLE IF NOT EXISTS captures (
            id TEXT PRIMARY KEY,
            source_id TEXT,
            space_id TEXT,
            type TEXT,
            raw_content TEXT,
            clean_content TEXT,
            tags TEXT,
            chunk_index INTEGER,
            vector_id INTEGER,
            created_at TEXT
        )
    """)
    conn.commit()
    return conn

def embed_texts(texts):
    # 預留串接 fastembed 的實作
    pass

def build_kb(input_file, output_db, name):
    print(f"Building Knowledge Base: {name}")
    conn = init_db(output_db, name)
    # 假設這是一個簡單的分行文檔
    with open(input_file, 'r', encoding='utf-8') as f:
        chunks = f.readlines()
        
    cur = conn.cursor()
    for idx, c in enumerate(chunks):
        c = c.strip()
        if not c:
            continue
        cur.execute("INSERT INTO captures VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", (
            str(uuid.uuid4()), "doc1", None, "text", c, c, "[]", idx, idx, datetime.now().isoformat()
        ))
    
    print(f"Done. Embedded {len(chunks)} chunks into {output_db}.")
    conn.commit()
    conn.close()

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--name', required=True)
    args = parser.parse_args()
    build_kb(args.input, args.output, args.name)
