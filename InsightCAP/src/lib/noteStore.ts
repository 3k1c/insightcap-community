// src/lib/noteStore.ts
// localStorage-backed note persistence for the editor
// Namespace: "notes" — keys like  notes:session, notes:files, notes:content:{id}, notes:history:{id}

export interface NoteFile {
    id: string;
    title: string;
    createdAt: number;
    updatedAt: number;
}

export interface HistoryEntry {
    ts: number;
    snapshot: string; // Tiptap JSON string
}

export interface NoteSession {
    activeTabId: string;
    tabs: Array<{ id: string; title: string }>;
    tabCounter: number;
}

const NS = 'notes';
const SESSION_KEY = `${NS}:session`;
const FILES_KEY = `${NS}:files`;
const MAX_HISTORY = 20;

const contentKey = (id: string) => `${NS}:content:${id}`;
const historyKey = (id: string) => `${NS}:history:${id}`;

// ── Session ────────────────────────────────────────────────────────────────────

export function loadSession(): NoteSession | null {
    try {
        const raw = localStorage.getItem(SESSION_KEY);
        if (raw) return JSON.parse(raw) as NoteSession;
    } catch {}
    return null;
}

export function saveSession(session: NoteSession): void {
    try { localStorage.setItem(SESSION_KEY, JSON.stringify(session)); } catch {}
}

// ── Content ────────────────────────────────────────────────────────────────────

export function loadContent(id: string): string {
    return localStorage.getItem(contentKey(id)) ?? '';
}

export function saveContent(id: string, content: string): void {
    try {
        localStorage.setItem(contentKey(id), content);
        updateFileTimestamp(id);
    } catch {}
}

// ── File Registry ──────────────────────────────────────────────────────────────

export function loadFiles(): NoteFile[] {
    try {
        const raw = localStorage.getItem(FILES_KEY);
        if (raw) return JSON.parse(raw) as NoteFile[];
    } catch {}
    return [];
}

export function saveFiles(files: NoteFile[]): void {
    try { localStorage.setItem(FILES_KEY, JSON.stringify(files)); } catch {}
}

export function upsertFile(file: NoteFile): void {
    const files = loadFiles();
    const idx = files.findIndex(f => f.id === file.id);
    if (idx >= 0) { files[idx] = file; } else { files.unshift(file); }
    saveFiles(files);
}

export function updateFileTimestamp(id: string): void {
    const files = loadFiles();
    const f = files.find(f => f.id === id);
    if (f) { f.updatedAt = Date.now(); saveFiles(files); }
}

export function renameFile(id: string, title: string): void {
    const files = loadFiles();
    const f = files.find(f => f.id === id);
    if (f) { f.title = title; f.updatedAt = Date.now(); saveFiles(files); }
}

export function deleteFile(id: string): void {
    localStorage.removeItem(contentKey(id));
    localStorage.removeItem(historyKey(id));
    saveFiles(loadFiles().filter(f => f.id !== id));
}

// ── History ────────────────────────────────────────────────────────────────────

// Tiptap empty doc — skip snapshotting this
const EMPTY_DOC = '{"type":"doc","content":[{"type":"paragraph"}]}';

export function loadHistory(id: string): HistoryEntry[] {
    try {
        const raw = localStorage.getItem(historyKey(id));
        if (raw) return JSON.parse(raw) as HistoryEntry[];
    } catch {}
    return [];
}

export function addHistory(id: string, snapshot: string): void {
    try {
        if (!snapshot || snapshot === EMPTY_DOC) return;
        const history = loadHistory(id);
        // Skip if same as most-recent snapshot
        if (history.length > 0 && history[0].snapshot === snapshot) return;
        history.unshift({ ts: Date.now(), snapshot });
        if (history.length > MAX_HISTORY) history.length = MAX_HISTORY;
        localStorage.setItem(historyKey(id), JSON.stringify(history));
    } catch {}
}
