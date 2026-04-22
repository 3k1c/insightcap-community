#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
清理歷史低訊號 URL chunks（預設 dry-run）。

用法：
  python scripts/cleanup-legacy-url-chunks.py --db "C:\\path\\to\\.insightcap\\insightcap.db"
  python scripts/cleanup-legacy-url-chunks.py --db "C:\\path\\to\\.insightcap\\insightcap.db" --apply
"""

from __future__ import annotations

import argparse
import sqlite3
from dataclasses import dataclass
from typing import Iterable


@dataclass
class Candidate:
    capture_id: str
    source_id: str | None
    clean_content: str


def is_low_signal_url_chunk(content: str) -> bool:
    text = (content or "").strip()
    if not text:
        return True

    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    if not lines:
        return True

    char_count = len(text)
    alpha_num_count = sum(1 for ch in text if ch.isalnum())
    url_line_count = sum(
        1 for ln in lines if ln.startswith("http://") or ln.startswith("https://")
    )
    short_plain_lines = sum(
        1
        for ln in lines
        if len(ln) <= 60
        and "." not in ln
        and "!" not in ln
        and "?" not in ln
        and "。" not in ln
        and "，" not in ln
        and "！" not in ln
        and "？" not in ln
    )

    if char_count < 120 and len(lines) <= 3:
        return True
    if alpha_num_count < 40:
        return True
    return len(lines) <= 4 and (short_plain_lines + url_line_count >= len(lines)) and char_count < 220


def load_candidates(conn: sqlite3.Connection) -> list[Candidate]:
    rows = conn.execute(
        """
        SELECT c.id, c.source_id, c.clean_content
        FROM captures c
        WHERE c.type = 'url' AND c.status = 'processed'
        """
    ).fetchall()
    return [
        Candidate(
            capture_id=str(r[0]),
            source_id=str(r[1]) if r[1] is not None else None,
            clean_content=str(r[2] or ""),
        )
        for r in rows
    ]


def filter_low_signal(candidates: Iterable[Candidate]) -> list[Candidate]:
    return [c for c in candidates if is_low_signal_url_chunk(c.clean_content)]


def apply_cleanup(conn: sqlite3.Connection, low_signal: list[Candidate], prune_empty_sources: bool) -> tuple[int, int]:
    if not low_signal:
        return 0, 0

    capture_ids = [c.capture_id for c in low_signal]
    source_ids = sorted({c.source_id for c in low_signal if c.source_id})

    with conn:
        conn.executemany(
            "DELETE FROM chunk_relations WHERE left_type = 'capture' AND left_chunk_id = ?",
            [(cid,) for cid in capture_ids],
        )
        conn.executemany(
            "DELETE FROM chunk_relations WHERE right_type = 'capture' AND right_chunk_id = ?",
            [(cid,) for cid in capture_ids],
        )
        conn.executemany("DELETE FROM captures WHERE id = ?", [(cid,) for cid in capture_ids])

        for sid in source_ids:
            conn.execute(
                """
                UPDATE sources
                SET capture_count = (
                    SELECT COUNT(*) FROM captures WHERE source_id = ?
                ),
                updated_at = datetime('now')
                WHERE id = ?
                """,
                (sid, sid),
            )

        deleted_sources = 0
        if prune_empty_sources:
            row = conn.execute(
                "SELECT COUNT(*) FROM sources WHERE capture_count = 0"
            ).fetchone()
            deleted_sources = int(row[0] if row else 0)
            conn.execute("DELETE FROM sources WHERE capture_count = 0")
    return len(capture_ids), deleted_sources


def main() -> int:
    parser = argparse.ArgumentParser(description="清理歷史低訊號 URL chunks（預設 dry-run）")
    parser.add_argument("--db", required=True, help="insightcap.db 的絕對路徑")
    parser.add_argument("--apply", action="store_true", help="實際執行刪除（不加則只預覽）")
    parser.add_argument(
        "--no-prune-empty-sources",
        action="store_true",
        help="保留沒有 captures 的 sources",
    )
    args = parser.parse_args()

    conn = sqlite3.connect(args.db)
    conn.row_factory = sqlite3.Row
    try:
        all_candidates = load_candidates(conn)
        low_signal = filter_low_signal(all_candidates)

        print(f"[scan] url processed chunks: {len(all_candidates)}")
        print(f"[scan] low-signal candidates: {len(low_signal)}")

        preview_n = min(12, len(low_signal))
        if preview_n > 0:
            print(f"[preview] first {preview_n} candidate ids:")
            for c in low_signal[:preview_n]:
                sid = c.source_id or "NULL"
                print(f"  - capture={c.capture_id} source={sid}")

        if not args.apply:
            print("[dry-run] no data changed. add --apply to execute cleanup.")
            return 0

        removed_chunks, removed_sources = apply_cleanup(
            conn=conn,
            low_signal=low_signal,
            prune_empty_sources=not args.no_prune_empty_sources,
        )
        print(f"[done] removed chunks: {removed_chunks}")
        print(f"[done] removed empty sources: {removed_sources}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())

