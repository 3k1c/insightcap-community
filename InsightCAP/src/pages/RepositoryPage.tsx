import React, { useEffect, useMemo, useRef, useState, useCallback, KeyboardEvent } from 'react';
import { Search, FileText, PlayCircle, ImageIcon, NotebookPen, X, Database, Trash2, Plus, Tag, Layers, Pencil, Check, ChevronDown, Globe, FileCode, FileSpreadsheet, File, BookText, Brain, Fingerprint, Bug, RefreshCw } from 'lucide-react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import { useKnowledgeStore, type TimelineSourceItem, type CaptureDetail, type RepositoryStats } from '../stores/knowledgeStore';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { loadFiles, loadContent, saveContent, upsertFile, deleteFile, type NoteFile } from '../lib/noteStore';
import { tauriCmd } from '../lib/tauri';
import { useTagStore } from '../stores/tagStore';
import { useT } from '../hooks/useT';


function formatDateLabel(key: string, t: (key: string) => string): string {
    const today = new Date().toISOString().slice(0, 10);
    if (key === today) return t('repository.today');
    const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
    if (key === yesterday) return t('repository.yesterday');
    const [, m, d] = key.split('-');
    return `${parseInt(m)}/${parseInt(d)}`;
}

function formatDateSub(key: string, t: (key: string) => string): string {
    if (key === 'unknown') return '';
    const today = new Date().toISOString().slice(0, 10);
    if (key === today || key === new Date(Date.now() - 86_400_000).toISOString().slice(0, 10)) return key;
    const days = [
        t('repository.weekday_sun'),
        t('repository.weekday_mon'),
        t('repository.weekday_tue'),
        t('repository.weekday_wed'),
        t('repository.weekday_thu'),
        t('repository.weekday_fri'),
        t('repository.weekday_sat'),
    ];
    const d = new Date(key);
    return isNaN(d.getTime()) ? '' : `${t('repository.week_prefix')}${days[d.getDay()]}`;
}

function safeDateKey(value: string | number | undefined | null): string {
    try {
        if (!value) return 'unknown';
        const d = typeof value === 'number' ? new Date(value) : new Date(value);
        const iso = d.toISOString().slice(0, 10);
        return iso === 'Invalid' ? 'unknown' : iso;
    } catch {
        return 'unknown';
    }
}

function safeTitle(t: string | undefined | null): string {
    if (!t) return '';

    let cleaned = t;

    cleaned = cleaned.replace(/\s*[-\u2014]\s*(modified|\u5df2\u4fee\u6539)$/i, '');

    const pdfIndex = cleaned.toLowerCase().lastIndexOf('.pdf');
    if (pdfIndex !== -1 && pdfIndex + 4 < cleaned.length) {
        const afterPdf = cleaned.slice(pdfIndex + 4);
        if (/^\s*[-\u2014|]/.test(afterPdf)) {
            cleaned = cleaned.slice(0, pdfIndex + 4);
        }
    }

    const appNames = [
        'visual studio code', 'vscode', 'insightcap',
        'adobe acrobat.*', 'waterfox', 'google chrome',
        'mozilla firefox', 'firefox', 'microsoft edge', 'edge',
        'brave', 'safari', 'opera', 'arc'
    ].join('|');

    const appRegex = new RegExp(`\\s*[-\\u2014|]\\s*(${appNames}|[a-zA-Z0-9-]+\\.(com|net|org|io|tw|hk|cn))\\s*$`, 'i');

    while (appRegex.test(cleaned)) {
        cleaned = cleaned.replace(appRegex, '');
    }

    return cleaned.replace(/\s{2,}/g, ' ').trim();
}

function inferPath(item: TimelineSourceItem): string | null {
    return item.localDocPath || item.filePath || null;
}

function inferOpenUrl(item: TimelineSourceItem): string | null {
    if (item.url && /^https?:\/\//i.test(item.url)) return item.url;

    const haystack = [item.url, item.title, item.contentPreview]
        .filter(Boolean)
        .join(' ');

    const directHttp = haystack.match(/https?:\/\/[^\s)>\]}]+/i)?.[0];
    if (directHttp) return directHttp;

    const ytId = haystack.match(/(?:youtu\.be\/|youtube\.com\/watch\?v=)([A-Za-z0-9_-]{6,})/i)?.[1];
    if (ytId) return `https://www.youtube.com/watch?v=${ytId}`;

    const bvId = haystack.match(/\b(BV[0-9A-Za-z]{10})\b/)?.[1];
    if (bvId) return `https://www.bilibili.com/video/${bvId}`;

    const domainUrl = haystack.match(/\b(?:www\.)?(?:youtube\.com\/watch\?v=[A-Za-z0-9_-]+|youtu\.be\/[A-Za-z0-9_-]+|bilibili\.com\/video\/[A-Za-z0-9_-]+|b23\.tv\/[A-Za-z0-9_-]+)\b/i)?.[0];
    if (domainUrl) return domainUrl.startsWith('http') ? domainUrl : `https://${domainUrl}`;

    return null;
}

type EmbeddedMedia =
    | { kind: 'youtube' | 'bilibili'; src: string }
    | { kind: 'video'; src: string };

function getEmbeddedMedia(item: TimelineSourceItem | undefined): EmbeddedMedia | null {
    if (!item) return null;
    const targetUrl = inferOpenUrl(item);
    if (!targetUrl) return null;

    const ytId = targetUrl.match(/(?:youtu\.be\/|youtube\.com\/(?:watch\?v=|shorts\/))([A-Za-z0-9_-]{6,})/i)?.[1];
    if (ytId) {
        return { kind: 'youtube', src: `https://www.youtube.com/embed/${ytId}` };
    }

    const bvId = targetUrl.match(/(?:bilibili\.com\/video\/)?(BV[0-9A-Za-z]{10})/i)?.[1];
    if (bvId) {
        return { kind: 'bilibili', src: `https://player.bilibili.com/player.html?bvid=${bvId}&high_quality=1&danmaku=0&autoplay=0` };
    }

    if (/\.(mp4|webm|ogg|mov)(\?|#|$)/i.test(targetUrl)) {
        return { kind: 'video', src: targetUrl };
    }

    return null;
}

function extractPlainText(raw: string): string {
    try {
        const doc = JSON.parse(raw);
        if (doc && doc.type === 'doc' && Array.isArray(doc.content)) {
            return walkNodes(doc.content).trim();
        }
        const extracted = extractReadableJsonText(doc);
        if (extracted) return extracted;
    } catch {
    }
    return raw;
}

function extractReadableJsonText(value: unknown): string {
    const seen = new Set<string>();
    const lines: string[] = [];

    const add = (text: string) => {
        const cleaned = text.replace(/\s+/g, ' ').trim();
        if (!cleaned || cleaned.length < 2 || seen.has(cleaned)) return;
        seen.add(cleaned);
        lines.push(cleaned);
    };

    const visit = (node: unknown, depth = 0) => {
        if (lines.length >= 80 || depth > 10 || node == null) return;

        if (typeof node === 'string') {
            add(node);
            return;
        }

        if (typeof node === 'number' || typeof node === 'boolean') return;

        if (Array.isArray(node)) {
            for (const item of node) visit(item, depth + 1);
            return;
        }

        if (typeof node === 'object') {
            const obj = node as Record<string, unknown>;
            const priorityKeys = [
                'title',
                'name',
                'description',
                'summary',
                'content',
                'text',
                'prompt',
                'structure',
                'position',
                'background',
            ];

            for (const key of priorityKeys) {
                if (key in obj) visit(obj[key], depth + 1);
            }

            for (const [key, child] of Object.entries(obj)) {
                if (priorityKeys.includes(key)) continue;
                visit(child, depth + 1);
            }
        }
    };

    visit(value);
    return lines.join('\n');
}

function walkNodes(nodes: unknown[]): string {
    let out = '';
    for (const node of nodes) {
        if (!node || typeof node !== 'object') continue;
        const n = node as Record<string, unknown>;
        if (n.type === 'text' && typeof n.text === 'string') {
            out += n.text;
        } else if (n.type === 'hardBreak') {
            out += '\n';
        } else if (Array.isArray(n.content)) {
            out += walkNodes(n.content);
        }
        if (['paragraph', 'heading', 'blockquote', 'codeBlock', 'listItem', 'bulletList', 'orderedList'].includes(n.type as string)) {
            out += '\n';
        }
    }
    return out;
}

const mediaColor: Record<string, string> = {
    url: 'text-blue-500',
    video: 'text-purple-500',
    image: 'text-emerald-500',
    pdf: 'text-red-400',
    word: 'text-blue-600 dark:text-blue-400',
    excel: 'text-emerald-600 dark:text-emerald-400',
    code: 'text-amber-500',
    text: 'text-text-secondary',
};

const mediaBg: Record<string, string> = {
    url: 'bg-blue-500/10 dark:bg-blue-500/20',
    video: 'bg-purple-500/10 dark:bg-purple-500/20',
    image: 'bg-emerald-500/10 dark:bg-emerald-500/20',
    pdf: 'bg-red-500/10 dark:bg-red-500/20',
    word: 'bg-blue-500/10 dark:bg-blue-500/20',
    excel: 'bg-emerald-500/10 dark:bg-emerald-500/20',
    code: 'bg-amber-500/10 dark:bg-amber-500/20',
    text: 'bg-surface-subtle',
};

function getMediaConfig(item: TimelineSourceItem) {
    let mt = item.mediaType || 'text';

    const pathStr = (item.filePath || item.localDocPath || item.title || '').toLowerCase();

    if (mt === 'url' || pathStr.startsWith('http')) {
        mt = 'url';
    } else if (pathStr.endsWith('.pdf')) {
        mt = 'pdf';
    } else if (/\.(doc|docx)$/.test(pathStr)) {
        mt = 'word';
    } else if (/\.(xls|xlsx|csv)$/.test(pathStr)) {
        mt = 'excel';
    } else if (/\.(js|ts|jsx|tsx|py|rs|go|c|cpp|h|java|json|html|css)$/.test(pathStr)) {
        mt = 'code';
    } else if (/\.(png|jpg|jpeg|gif|webp|svg)$/.test(pathStr)) {
        mt = 'image';
    } else if (/\.(mp4|mov|avi|webm)$/.test(pathStr)) {
        mt = 'video';
    }

    const iconColor = mediaColor[mt] || mediaColor.text;
    const iconBg = mediaBg[mt] || mediaBg.text;

    let icon = <File className="h-6 w-6" />;
    if (mt === 'url') icon = <Globe className="h-6 w-6" />;
    else if (mt === 'video') icon = <PlayCircle className="h-6 w-6" />;
    else if (mt === 'image') icon = <ImageIcon className="h-6 w-6" />;
    else if (mt === 'pdf') icon = <BookText className="h-6 w-6" />;
    else if (mt === 'word') icon = <FileText className="h-6 w-6" />;
    else if (mt === 'excel') icon = <FileSpreadsheet className="h-6 w-6" />;
    else if (mt === 'code') icon = <FileCode className="h-6 w-6" />;

    return { mt, iconColor, iconBg, icon };
}

function getVideoThumbnail(item: TimelineSourceItem): string | null {
    if (item.thumbnail) return item.thumbnail;

    const targetUrl = inferOpenUrl(item);
    if (!targetUrl) return null;

    const ytId = targetUrl.match(/(?:youtu\.be\/|youtube\.com\/(?:watch\?v=|shorts\/))([A-Za-z0-9_-]{6,})/i)?.[1];
    if (ytId) {
        return `https://i.ytimg.com/vi/${ytId}/hqdefault.jpg`;
    }

    return null;
}

function extractBvidFromItem(item: TimelineSourceItem): string | null {
    const targetUrl = inferOpenUrl(item) || '';
    const fromUrl = targetUrl.match(/(?:bilibili\.com\/video\/)?(BV[0-9A-Za-z]{10})/i)?.[1];
    if (fromUrl) return fromUrl;

    const haystack = [item.url, item.title, item.contentPreview].filter(Boolean).join(' ');
    const fromText = haystack.match(/\b(BV[0-9A-Za-z]{10})\b/i)?.[1];
    return fromText || null;
}

const bilibiliCoverCache = new Map<string, string | null>();

async function fetchBilibiliCoverByBvid(bvid: string): Promise<string | null> {
    if (bilibiliCoverCache.has(bvid)) {
        return bilibiliCoverCache.get(bvid) ?? null;
    }

    try {
        const res = await fetch(`https://api.bilibili.com/x/web-interface/view?bvid=${encodeURIComponent(bvid)}`);
        if (!res.ok) {
            bilibiliCoverCache.set(bvid, null);
            return null;
        }
        const json = await res.json();
        const cover = typeof json?.data?.pic === 'string' ? json.data.pic : null;
        bilibiliCoverCache.set(bvid, cover);
        return cover;
    } catch {
        bilibiliCoverCache.set(bvid, null);
        return null;
    }
}


interface DayGroup {
    dateKey: string;
    sourceItems: TimelineSourceItem[];
    noteItems: NoteFile[];
}

const EMPTY_NOTE_DOC = '{"type":"doc","content":[{"type":"paragraph"}]}';

interface PreviewDoc {
    title: string;
    content: string;
    sourceId?: string;
    sourceItem?: TimelineSourceItem;
}

type ImportTaskStatus = 'queued' | 'processing' | 'done' | 'failed';

interface ImportTaskItem {
    id: string;
    filePath: string;
    fileName: string;
    stage: string;
    status: ImportTaskStatus;
    current: number;
    total: number;
    message: string;
}

interface ImportTaskProgressPayload {
    taskId?: string | null;
    filePath: string;
    fileName: string;
    stage: string;
    status: ImportTaskStatus;
    current: number;
    total: number;
    message: string;
}

function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).pop() || path;
}

function createImportTaskId(index: number): string {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
        return crypto.randomUUID();
    }
    return `import-${Date.now()}-${index}`;
}

function importStageLabel(stage: string, t: (key: string) => string): string {
    const labels: Record<string, string> = {
        queued: t('repository.import_stage_queued'),
        parsing: t('repository.import_stage_parsing'),
        cleaning: t('repository.import_stage_cleaning'),
        saving: t('repository.import_stage_saving'),
        indexing: t('repository.import_stage_indexing'),
        completed: t('repository.import_stage_completed'),
        failed: t('repository.import_stage_failed'),
    };
    return labels[stage] || stage;
}


export const RepositoryPage: React.FC = () => {
    const t = useT();
    const [query, setQuery] = useState('');
    const [notes, setNotes] = useState<NoteFile[]>([]);
    const [selectedDateKey, setSelectedDateKey] = useState<string | null>(null);
    const [docPreview, setDocPreview] = useState<PreviewDoc | null>(null);
    const [imagePreview, setImagePreview] = useState<{ src: string; title: string } | null>(null);
    const [previewChunks, setPreviewChunks] = useState<CaptureDetail[]>([]);
    const [isLoadingChunks, setIsLoadingChunks] = useState(false);
    const [failedEmbeddedUrls, setFailedEmbeddedUrls] = useState<Set<string>>(() => new Set());
    const [bilibiliCovers, setBilibiliCovers] = useState<Record<string, string>>({});
    const [typeFilter, setTypeFilter] = useState<'all' | 'source' | 'note'>('all');
    const [selectedTag, setSelectedTag] = useState<string | null>(null);
    const [spaceDropdownOpen, setSpaceDropdownOpen] = useState(false);
    const [importTasks, setImportTasks] = useState<ImportTaskItem[]>([]);
    const [importPanelExpanded, setImportPanelExpanded] = useState(true);
    const scrollerRef = useRef<HTMLDivElement | null>(null);
    const sectionRefs = useRef<Record<string, HTMLElement | null>>({});
    const firstRowRefs = useRef<Record<string, HTMLDivElement | null>>({});
    const markerRefs = useRef<Record<string, HTMLSpanElement | null>>({});

    const timelineSources = useKnowledgeStore((s) => s.timelineSources);
    const isLoadingTimeline = useKnowledgeStore((s) => s.isLoadingTimeline);
    const loadTimeline = useKnowledgeStore((s) => s.loadTimeline);
    const deleteSource = useKnowledgeStore((s) => s.deleteSource);
    const updateCapture = useKnowledgeStore((s) => s.updateCapture);
    const spaces = useKnowledgeStore((s) => s.spaces);
    const loadSpaces = useKnowledgeStore((s) => s.loadSpaces);
    const createSpace = useKnowledgeStore((s) => s.createSpace);
    const spaceFilter = useKnowledgeStore((s) => s.spaceFilter);
    const setSpaceFilter = useKnowledgeStore((s) => s.setSpaceFilter);
    const titleOf = (value: string | undefined | null) => safeTitle(value) || t('repository.untitled');
    const embeddedMedia = useMemo(() => getEmbeddedMedia(docPreview?.sourceItem), [docPreview]);

    const [repoStats, setRepoStats] = useState<RepositoryStats>({
        todaySources: 0,
        totalChunks: 0,
        totalTags: 0,
        totalData: 0,
        totalPatterns: 0,
        totalLogs: 0,
    });
    const [isRefreshingClassification, setIsRefreshingClassification] = useState(false);

    const [editingChunkId, setEditingChunkId] = useState<string | null>(null);
    const [editSpaceId, setEditSpaceId] = useState<string>('');
    const [editTags, setEditTags] = useState<string[]>([]);
    const [editTagInput, setEditTagInput] = useState('');
    const [isSavingChunk, setIsSavingChunk] = useState(false);

    const startEditChunk = useCallback((chunk: CaptureDetail) => {
        setEditingChunkId(chunk.id);
        setEditSpaceId(chunk.spaceId ?? '');
        try {
            const parsed = JSON.parse(chunk.tags);
            setEditTags(Array.isArray(parsed) ? parsed : []);
        } catch {
            setEditTags([]);
        }
        setEditTagInput('');
    }, []);

    const cancelEditChunk = useCallback(() => {
        setEditingChunkId(null);
        setEditTagInput('');
    }, []);

    const handleTagKeyDown = useCallback((e: KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter' && editTagInput.trim()) {
            e.preventDefault();
            const tag = editTagInput.trim().replace(/^#/, '');
            if (tag && !editTags.includes(tag)) {
                setEditTags(prev => [...prev, tag]);
            }
            setEditTagInput('');
        } else if (e.key === 'Backspace' && !editTagInput && editTags.length > 0) {
            setEditTags(prev => prev.slice(0, -1));
        }
    }, [editTagInput, editTags]);

    const saveEditChunk = useCallback(async (chunkId: string) => {
        setIsSavingChunk(true);
        try {
            const nextTagsJson = JSON.stringify(editTags);
            const nextSpaceId = editSpaceId || undefined;
            await updateCapture(chunkId, undefined, nextTagsJson, nextSpaceId);
            setPreviewChunks((prev) =>
                prev.map((c) =>
                    c.id === chunkId
                        ? {
                            ...c,
                            tags: nextTagsJson,
                            spaceId: nextSpaceId ?? c.spaceId,
                            isUserEdited: true,
                        }
                        : c,
                ),
            );
            setEditingChunkId(null);
            toast.success(t('repository.chunk_save_success'));
        } finally {
            setIsSavingChunk(false);
        }
    }, [editTags, editSpaceId, updateCapture, t]);

    const handleCreateNewSpace = useCallback(async () => {
        const name = window.prompt(t('repository.new_space_prompt') || 'Enter new space name:');
        if (name && name.trim()) {
            const newId = await createSpace(name.trim());
            if (newId) {
                setEditSpaceId(newId);
                toast.success(t('repository.new_space_success') || 'New space created');
            }
        }
    }, [createSpace, t]);

    const recentTags = useTagStore((s) => s.recentTags);
    const loadRecentTags = useTagStore((s) => s.loadRecentTags);

    const loadRepositoryStats = useCallback(async () => {
        const stats = await invoke<Partial<RepositoryStats>>('get_repository_stats');
        setRepoStats((prev) => ({ ...prev, ...stats }));
    }, []);

    useEffect(() => {
        loadTimeline();
        loadRecentTags();
        loadSpaces();
        try { setNotes(loadFiles()); } catch { /* ignore */ }
        loadRepositoryStats().catch(console.error);
    }, [loadTimeline, loadRecentTags, loadSpaces, loadRepositoryStats]);

    useEffect(() => {
        let disposed = false;
        const handleProgress = (event: { payload: ImportTaskProgressPayload }) => {
            const payload = event.payload;
            const taskId = payload.taskId || payload.filePath || payload.fileName;
            if (!taskId) return;
            setImportTasks((prev) =>
                prev.some((task) => task.id === taskId || task.filePath === payload.filePath)
                    ? prev.map((task) =>
                        task.id === taskId || task.filePath === payload.filePath
                            ? {
                                ...task,
                                filePath: payload.filePath || task.filePath,
                                fileName: payload.fileName || task.fileName,
                                stage: payload.stage || task.stage,
                                status: payload.status,
                                current: payload.current ?? task.current,
                                total: payload.total ?? task.total,
                                message: payload.message || task.message,
                            }
                            : task,
                    )
                    : [
                        ...prev,
                        {
                            id: taskId,
                            filePath: payload.filePath,
                            fileName: payload.fileName || fileNameFromPath(payload.filePath),
                            stage: payload.stage || 'queued',
                            status: payload.status,
                            current: payload.current ?? 0,
                            total: payload.total ?? 0,
                            message: payload.message || '',
                        },
                    ],
            );
        };
        const importUnlistenPromise = listen<ImportTaskProgressPayload>('import-task-progress', handleProgress);
        const processingUnlistenPromise = listen<ImportTaskProgressPayload>('processing-task-progress', handleProgress);

        return () => {
            disposed = true;
            importUnlistenPromise.then((unlisten) => {
                if (disposed) unlisten();
            }).catch(console.error);
            processingUnlistenPromise.then((unlisten) => {
                if (disposed) unlisten();
            }).catch(console.error);
        };
    }, []);

    const handleRefreshClassification = useCallback(async () => {
        setIsRefreshingClassification(true);
        try {
            const updated = await invoke<number>('trigger_space_recluster');
            await Promise.all([loadSpaces(), loadTimeline(), loadRepositoryStats()]);
            toast.success(t('repository.refresh_classification_success', { count: updated ?? 0 }));
        } catch (error) {
            console.error('Failed to refresh classification:', error);
            toast.error(t('repository.refresh_classification_failed'));
        } finally {
            setIsRefreshingClassification(false);
        }
    }, [loadSpaces, loadTimeline, loadRepositoryStats, t]);

    useEffect(() => {
        const bvids = Array.from(
            new Set(
                timelineSources
                    .map((item) => extractBvidFromItem(item))
                    .filter((v): v is string => Boolean(v)),
            ),
        );
        const missing = bvids.filter((bvid) => !bilibiliCovers[bvid]);
        if (missing.length === 0) return;

        let canceled = false;
        for (const bvid of missing) {
            const cached = bilibiliCoverCache.get(bvid);
            if (cached) {
                setBilibiliCovers((prev) => (prev[bvid] ? prev : { ...prev, [bvid]: cached }));
                continue;
            }
            if (cached === null) continue;

            fetchBilibiliCoverByBvid(bvid).then((cover) => {
                if (!cover || canceled) return;
                setBilibiliCovers((prev) => (prev[bvid] ? prev : { ...prev, [bvid]: cover }));
            });
        }

        return () => {
            canceled = true;
        };
    }, [timelineSources, bilibiliCovers]);


    const totalChunks = useMemo(
        () => (Array.isArray(timelineSources) ? timelineSources : []).reduce((sum, s) => sum + (s.captureCount || 0), 0),
        [timelineSources],
    );

    const importProgress = useMemo(() => {
        const total = importTasks.length;
        const done = importTasks.filter((task) => task.status === 'done').length;
        const failed = importTasks.filter((task) => task.status === 'failed').length;
        const active = importTasks.find((task) => task.status === 'processing') ?? importTasks.find((task) => task.status === 'queued') ?? null;
        return {
            total,
            done,
            failed,
            active,
            isRunning: importTasks.some((task) => task.status === 'queued' || task.status === 'processing'),
            percent: total > 0 ? Math.round(((done + failed) / total) * 100) : 0,
        };
    }, [importTasks]);

    useEffect(() => {
        if (importProgress.total === 0 || importProgress.isRunning || importProgress.failed > 0) return;
        const timer = window.setTimeout(() => {
            setImportTasks([]);
        }, 4000);
        return () => window.clearTimeout(timer);
    }, [importProgress.failed, importProgress.isRunning, importProgress.total]);

    const topTags = useMemo(() => {
        const sorted = [...recentTags].sort((a, b) => b.recentCount - a.recentCount || b.useCount - a.useCount);
        return sorted.slice(0, 5);
    }, [recentTags]);


    const keyword = query.trim().toLowerCase();

    const filteredSources = useMemo(() => {
        const arr = Array.isArray(timelineSources) ? timelineSources : [];
        if (!keyword) return arr;
        return arr.filter((item) => {
            const hay = [item.title, item.contentPreview, item.filePath, item.url]
                .filter(Boolean)
                .join(' ')
                .toLowerCase();
            return hay.includes(keyword);
        });
    }, [keyword, timelineSources]);

    const filteredNotes = useMemo(() => {
        const arr = Array.isArray(notes) ? notes : [];
        if (!keyword) return arr;
        return arr.filter((n) => safeTitle(n.title).toLowerCase().includes(keyword));
    }, [keyword, notes]);


    const dayGroups = useMemo<DayGroup[]>(() => {
        const map = new Map<string, DayGroup>();

        for (const s of filteredSources) {
            const key = safeDateKey(s.capturedAt);
            if (!map.has(key)) map.set(key, { dateKey: key, sourceItems: [], noteItems: [] });
            map.get(key)!.sourceItems.push(s);
        }

        for (const n of filteredNotes) {
            const key = safeDateKey(n.updatedAt);
            if (!map.has(key)) map.set(key, { dateKey: key, sourceItems: [], noteItems: [] });
            map.get(key)!.noteItems.push(n);
        }

        return [...map.values()].sort((a, b) => b.dateKey.localeCompare(a.dateKey));
    }, [filteredSources, filteredNotes]);

    useEffect(() => {
        if (dayGroups.length === 0) {
            if (selectedDateKey !== null) setSelectedDateKey(null);
            return;
        }

        if (!selectedDateKey || !dayGroups.some((group) => group.dateKey === selectedDateKey)) {
            setSelectedDateKey(dayGroups[0].dateKey);
        }
    }, [dayGroups, selectedDateKey]);

    useEffect(() => {
        const scroller = scrollerRef.current;
        if (!scroller || dayGroups.length === 0) return;

        let rafId = 0;

        const updateActiveDate = () => {
            const scrollerRect = scroller.getBoundingClientRect();
            const viewportCenterY = scrollerRect.top + scroller.clientHeight / 2;

            let bestKey: string | null = null;
            let bestDistance = Number.POSITIVE_INFINITY;

            for (const group of dayGroups) {
                const firstRow = firstRowRefs.current[group.dateKey];
                const section = sectionRefs.current[group.dateKey];
                const targetEl = (() => {
                    if (firstRow) {
                        const firstCard = firstRow.querySelector('button');
                        if (firstCard instanceof HTMLElement) return firstCard;
                        return firstRow;
                    }
                    return section;
                })();

                if (!targetEl) continue;

                const rect = targetEl.getBoundingClientRect();
                const centerY = rect.top + rect.height / 2;
                const distance = Math.abs(centerY - viewportCenterY);

                if (distance < bestDistance) {
                    bestDistance = distance;
                    bestKey = group.dateKey;
                }
            }

            if (bestKey) {
                setSelectedDateKey((prev) => (prev === bestKey ? prev : bestKey));
            }
        };

        const onScroll = () => {
            if (rafId) cancelAnimationFrame(rafId);
            rafId = requestAnimationFrame(updateActiveDate);
        };

        scroller.addEventListener('scroll', onScroll, { passive: true });
        window.addEventListener('resize', onScroll);
        onScroll();

        return () => {
            scroller.removeEventListener('scroll', onScroll);
            window.removeEventListener('resize', onScroll);
            if (rafId) cancelAnimationFrame(rafId);
        };
    }, [dayGroups]);


    const handleOpenSource = async (item: TimelineSourceItem) => {
        const path = inferPath(item);
        const sourceUrl = inferOpenUrl(item);
        if (sourceUrl) {
            setFailedEmbeddedUrls((prev) => {
                if (!prev.has(sourceUrl)) return prev;
                const next = new Set(prev);
                next.delete(sourceUrl);
                return next;
            });
        }

        if (item.mediaType === 'image' && path) {
            setImagePreview({ src: convertFileSrc(path), title: titleOf(item.title) });
            return;
        }

        if (path) {
            try {
                const content = await tauriCmd.openDocument(path);
                setDocPreview({
                    title: titleOf(item.title),
                    content: extractPlainText(content),
                    sourceId: item.id,
                    sourceItem: item,
                });
            } catch {
                setDocPreview({
                    title: titleOf(item.title),
                    content: extractPlainText(item.contentPreview || t('repository.no_content')),
                    sourceId: item.id,
                    sourceItem: item,
                });
            }
            return;
        }

        setDocPreview({
            title: titleOf(item.title),
            content: extractPlainText(item.contentPreview || t('repository.no_content')),
            sourceId: item.id,
            sourceItem: item,
        });
    };

    const handleOpenNote = (note: NoteFile) => {
        const raw = loadContent(note.id) || t('repository.note_empty');
        setDocPreview({ title: titleOf(note.title), content: extractPlainText(raw), sourceId: undefined, sourceItem: undefined });
    };

    const handleOpenOriginalFromPreview = async () => {
        if (!docPreview?.sourceItem) return;
        const targetUrl = inferOpenUrl(docPreview.sourceItem);
        const path = inferPath(docPreview.sourceItem);

        if (targetUrl) {
            try {
                await openUrl(targetUrl);
            } catch (err) {
                console.error('Failed to open original URL in browser:', err);
                toast.error(t('repository.open_original_url_failed'));
            }
            return;
        }
        if (path) {
            try {
                await tauriCmd.openFileInSystem(path);
            } catch (err) {
                console.error('Failed to open original file in system:', err);
                toast.error(t('repository.open_original_file_failed'));
            }
            return;
        }

        toast.error(t('repository.open_original_missing'));
    };

    const handleDeleteSource = async (e: React.MouseEvent, item: TimelineSourceItem) => {
        e.stopPropagation();
        if (!window.confirm(t('repository.confirm_delete_source', { title: titleOf(item.title) }))) return;
        await deleteSource(item.id);
    };

    const handleDeleteNote = (e: React.MouseEvent, note: NoteFile) => {
        e.stopPropagation();
        if (!window.confirm(t('repository.confirm_delete_note', { title: titleOf(note.title) }))) return;
        deleteFile(note.id);
        setNotes(loadFiles());
    };

    const handleAddTodayDocument = async () => {
        try {
            const selected = await openDialog({
                multiple: true,
                filters: [{
                    name: t('repository.file_label'),
                    extensions: ['txt', 'md', 'doc', 'docx', 'xlsx', 'csv', 'pptx', 'pdf', 'py', 'js', 'ts', 'jsx', 'tsx', 'swift', 'rs', 'go', 'java', 'cpp', 'c', 'h', 'rb', 'php', 'html', 'wav', 'mp3', 'm4a', 'aac', 'flac', 'ogg', 'opus', 'webm'],
                }],
            });

            const paths = selected === null ? [] : Array.isArray(selected) ? selected : [selected];
            if (paths.length === 0) return;

            const tasks: ImportTaskItem[] = paths.map((path, index) => ({
                id: createImportTaskId(index),
                filePath: path,
                fileName: fileNameFromPath(path),
                stage: 'queued',
                status: 'queued',
                current: 0,
                total: 0,
                message: '',
            }));
            setImportTasks(tasks);
            setImportPanelExpanded(true);

            const results = await Promise.allSettled(
                tasks.map((task) =>
                    tauriCmd.ingestFile(task.filePath, undefined, task.id)
                        .then(() => {
                            setImportTasks((prev) =>
                                prev.map((item) =>
                                    item.id === task.id && item.status !== 'done'
                                        ? {
                                            ...item,
                                            stage: 'completed',
                                            status: 'done',
                                            current: item.total || 1,
                                            total: item.total || 1,
                                        }
                                        : item,
                                ),
                            );
                        })
                        .catch((error) => {
                            setImportTasks((prev) =>
                                prev.map((item) =>
                                    item.id === task.id
                                        ? {
                                            ...item,
                                            stage: 'failed',
                                            status: 'failed',
                                            message: String(error ?? ''),
                                        }
                                        : item,
                                ),
                            );
                            throw error;
                        }),
                ),
            );
            const successCount = results.filter((r) => r.status === 'fulfilled').length;
            const failures = results
                .map((result, index) => ({ result, path: paths[index] }))
                .filter((entry): entry is { result: PromiseRejectedResult; path: string } => entry.result.status === 'rejected');
            const failCount = failures.length;

            if (successCount > 0) {
                toast.success(t('repository.toast_import_success', { count: successCount }));
            }
            if (failCount > 0) {
                failures.forEach(({ result, path }) => {
                    console.error('[Repository] Failed to import file:', path, result.reason);
                });
                const firstError = String(failures[0].result.reason ?? '');
                toast.error(`${t('repository.toast_import_failed', { count: failCount })}${firstError ? `: ${firstError}` : ''}`);
            }

            await loadTimeline();
        } catch (error) {
            console.error('Failed to import files:', error);
            toast.error(t('repository.toast_open_dialog_failed'));
        }
    };

    const handleAddTodayNote = () => {
        const now = Date.now();
        const note: NoteFile = {
            id: `note-${now}`,
            title: t('repository.add_document'),
            createdAt: now,
            updatedAt: now,
        };
        upsertFile(note);
        saveContent(note.id, EMPTY_NOTE_DOC);
        setNotes(loadFiles());
        setDocPreview({
            title: titleOf(note.title),
            content: '',
            sourceId: undefined,
            sourceItem: undefined,
        });
    };

    const handleSelectDate = (dateKey: string) => {
        setSelectedDateKey(dateKey);
        const scroller = scrollerRef.current;
        const marker = markerRefs.current[dateKey];
        const firstRow = firstRowRefs.current[dateKey];
        const fallback = sectionRefs.current[dateKey];

        if (!scroller) return;

        if (!marker) {
            const target = firstRow || fallback;
            if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
            return;
        }

        const targetEl = (() => {
            if (firstRow) {
                const firstCard = firstRow.querySelector('button');
                if (firstCard instanceof HTMLElement) return firstCard;
                return firstRow;
            }
            return fallback;
        })();

        if (!targetEl) return;

        const markerRect = marker.getBoundingClientRect();
        const targetRect = targetEl.getBoundingClientRect();
        const markerCenterY = markerRect.top + markerRect.height / 2;
        const targetCenterY = targetRect.top + targetRect.height / 2;
        const nextTop = scroller.scrollTop + (targetCenterY - markerCenterY);

        scroller.scrollTo({ top: Math.max(0, nextTop), behavior: 'smooth' });
    };


    const todayKey = new Date().toISOString().slice(0, 10);
    const shouldShowTimelineActions =
        !keyword &&
        !selectedTag &&
        !spaceFilter;
    const shouldShowSourceAction = shouldShowTimelineActions && typeFilter === 'source';
    const shouldShowNoteAction = shouldShowTimelineActions && typeFilter === 'note';
    const shouldShowTodayActionGroup = shouldShowSourceAction || shouldShowNoteAction;

    const [tagSourceIds, setTagSourceIds] = useState<Set<string> | null>(null);

    useEffect(() => {
        if (!selectedTag) {
            setTagSourceIds(null);
            return;
        }
        let cancelled = false;
        invoke<string[]>('get_source_ids_by_tag', { tag: selectedTag }).then((ids) => {
            if (!cancelled) setTagSourceIds(new Set(ids));
        }).catch(() => {
            if (!cancelled) setTagSourceIds(new Set());
        });
        return () => { cancelled = true; };
    }, [selectedTag]);

    const visibleGroups = useMemo(() => {
        let groups = dayGroups;

        if (shouldShowTodayActionGroup && !groups.some((group) => group.dateKey === todayKey)) {
            groups = [{ dateKey: todayKey, sourceItems: [], noteItems: [] }, ...groups];
        }

        if (typeFilter !== 'all') {
            groups = groups
                .map((g) => ({
                    ...g,
                    sourceItems: typeFilter === 'source' ? g.sourceItems : [],
                    noteItems: typeFilter === 'note' ? g.noteItems : [],
                }))
                .filter((g) => g.sourceItems.length > 0 || g.noteItems.length > 0 || (shouldShowTodayActionGroup && g.dateKey === todayKey));
        }

        if (tagSourceIds) {
            groups = groups
                .map((g) => ({
                    ...g,
                    sourceItems: g.sourceItems.filter((s) => tagSourceIds.has(s.id)),
                }))
                .filter((g) => g.sourceItems.length > 0 || g.noteItems.length > 0 || (shouldShowTodayActionGroup && g.dateKey === todayKey));
        }

        return groups;
    }, [dayGroups, shouldShowTodayActionGroup, typeFilter, todayKey, tagSourceIds]);

    const importProgressPanel = importProgress.total > 0 ? (
        <div className="mb-5 rounded-xl border border-stroke-card bg-surface-layer px-4 py-3 shadow-sm">
            <div className="flex flex-wrap items-center gap-3">
                <button
                    type="button"
                    onClick={() => setImportPanelExpanded((open) => !open)}
                    className="flex min-w-0 flex-1 items-center gap-2 text-left"
                >
                    <ChevronDown className={`h-4 w-4 shrink-0 text-text-tertiary transition-transform ${importPanelExpanded ? 'rotate-180' : ''}`} />
                    <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-2">
                            <span className="text-fs-sm font-semibold text-text-primary">{t('repository.import_progress_title')}</span>
                            <span className="rounded-full bg-accent-default/10 px-2 py-0.5 text-fs-xs font-semibold text-accent-default">
                                {importProgress.done + importProgress.failed}/{importProgress.total}
                            </span>
                            {importProgress.failed > 0 && (
                                <span className="rounded-full bg-red-500/10 px-2 py-0.5 text-fs-xs font-semibold text-red-500">
                                    {t('repository.import_progress_failed', { count: importProgress.failed })}
                                </span>
                            )}
                        </div>
                        {importProgress.active && (
                            <p className="mt-0.5 truncate text-fs-xs text-text-tertiary">
                                {importStageLabel(importProgress.active.stage, t)} · {importProgress.active.fileName}
                            </p>
                        )}
                    </div>
                </button>
                {!importProgress.isRunning && (
                    <button
                        type="button"
                        onClick={() => setImportTasks([])}
                        className="rounded-full p-1 text-text-tertiary transition-colors hover:bg-surface-hover hover:text-text-primary"
                        aria-label={t('common.close')}
                    >
                        <X className="h-4 w-4" />
                    </button>
                )}
            </div>
            <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-surface-subtle">
                <div
                    className={`h-full rounded-full transition-all duration-300 ${importProgress.failed > 0 && !importProgress.isRunning ? 'bg-red-500' : 'bg-accent-default'}`}
                    style={{ width: `${importProgress.percent}%` }}
                />
            </div>
            {importPanelExpanded && (
                <div className="mt-3 max-h-44 space-y-1 overflow-y-auto pr-1">
                    {importTasks.map((task) => (
                        <div key={task.id} className="flex items-center gap-3 rounded-lg px-2 py-1.5 text-fs-xs hover:bg-surface-hover">
                            <span className={`h-2 w-2 shrink-0 rounded-full ${task.status === 'done'
                                ? 'bg-emerald-500'
                                : task.status === 'failed'
                                    ? 'bg-red-500'
                                    : task.status === 'processing'
                                        ? 'bg-accent-default'
                                        : 'bg-text-tertiary/40'
                                }`}
                            />
                            <span className="min-w-0 flex-1 truncate text-text-secondary">{task.fileName}</span>
                            <span className="shrink-0 text-text-tertiary">
                                {task.total > 0 && task.status === 'processing'
                                    ? `${importStageLabel(task.stage, t)} ${task.current}/${task.total}`
                                    : importStageLabel(task.stage, t)}
                            </span>
                        </div>
                    ))}
                </div>
            )}
        </div>
    ) : null;

    return (
        <div ref={scrollerRef} className="flex-1 overflow-auto bg-surface-base">
            <div className="sticky top-0 z-10 border-b border-stroke-divider bg-surface-base px-6 pb-4 pt-6">
                <div className="w-full">
                    <div className="mb-4 flex items-center gap-3">
                        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-accent-light2">
                            <Database className="h-4.5 w-4.5 text-accent-default" />
                        </div>
                        <h1 className="font-ui text-fs-xl font-bold text-text-primary">{t('repository.page_title')}</h1>

                        <div className="ml-auto flex items-center gap-2">
                            {[
                                { label: t('repository.stat_sources'), count: filteredSources.length, icon: <FileText className="h-3.5 w-3.5" />, color: 'text-blue-500 bg-blue-500/10' },
                                { label: t('repository.stat_notes'), count: filteredNotes.length, icon: <NotebookPen className="h-3.5 w-3.5" />, color: 'text-amber-500 bg-amber-500/10' },
                                { label: t('repository.stat_chunks'), count: totalChunks, icon: <Layers className="h-3.5 w-3.5" />, color: 'text-purple-500 bg-purple-500/10' },
                                { label: t('repository.stat_tags'), count: repoStats.totalTags, icon: <Tag className="h-3.5 w-3.5" />, color: 'text-emerald-500 bg-emerald-500/10' },
                                { label: t('repository.stat_spaces'), count: spaces.length, icon: <Database className="h-3.5 w-3.5" />, color: 'text-sky-500 bg-sky-500/10' },
                                { label: t('repository.stat_total_data'), count: repoStats.totalData, icon: <Brain className="h-3.5 w-3.5" />, color: 'text-cyan-500 bg-cyan-500/10' },
                                { label: t('repository.stat_total_patterns'), count: repoStats.totalPatterns, icon: <Fingerprint className="h-3.5 w-3.5" />, color: 'text-indigo-500 bg-indigo-500/10' },
                                { label: t('repository.stat_total_logs'), count: repoStats.totalLogs, icon: <Bug className="h-3.5 w-3.5" />, color: 'text-rose-500 bg-rose-500/10' },
                            ].map((s) => (
                                <div key={s.label} className="flex items-center gap-1.5 rounded-lg border border-stroke-card bg-surface-layer px-2.5 py-1.5 shadow-sm">
                                    <span className={`flex h-5 w-5 items-center justify-center rounded-md ${s.color}`}>{s.icon}</span>
                                    <span className="text-fs-base font-semibold tabular-nums text-text-primary">{s.count}</span>
                                    <span className="text-fs-xs text-text-tertiary">{s.label}</span>
                                </div>
                            ))}
                        </div>
                    </div>
                    <label className="relative block">
                        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-tertiary" />
                        <input
                            type="text"
                            value={query}
                            onChange={(e) => setQuery(e.target.value)}
                            placeholder={t('repository.search_placeholder')}
                            className="h-10 w-full rounded-xl border border-stroke-control bg-surface-control pl-10 pr-3 text-fs-base text-text-primary outline-none transition-all placeholder:text-text-tertiary focus:border-accent-default focus:ring-2 focus:ring-accent-default/20"
                        />
                        {query && (
                            <button
                                onClick={() => setQuery('')}
                                className="absolute right-3 top-1/2 -translate-y-1/2 rounded-md p-0.5 text-text-tertiary hover:text-text-secondary"
                            >
                                <X className="h-3.5 w-3.5" />
                            </button>
                        )}
                    </label>

                    <div className="mt-3 flex items-center gap-2">
                        <div className="flex min-w-0 flex-1 items-center gap-2 overflow-x-auto scrollbar-none">
                            {(['source', 'note', 'all'] as const).map((f) => {
                                const label = f === 'all' ? t('repository.filter_all') : f === 'source' ? t('repository.filter_source') : t('repository.filter_note');
                                const active = typeFilter === f;
                                return (
                                    <button
                                        key={f}
                                        type="button"
                                        onClick={() => setTypeFilter(f)}
                                        className={`shrink-0 rounded-full px-3 py-1 text-fs-xs font-semibold transition-colors ${active
                                            ? 'bg-accent-default text-white'
                                            : 'bg-surface-subtle text-text-secondary hover:text-text-primary'
                                            }`}
                                    >
                                        {label}
                                    </button>
                                );
                            })}

                            {topTags.length > 0 && (
                                <>
                                    <div className="mx-1 h-4 w-px shrink-0 bg-stroke-divider" />
                                    {topTags.map((tag) => {
                                        const active = selectedTag === tag.name;
                                        return (
                                            <button
                                                key={tag.id}
                                                type="button"
                                                onClick={() => setSelectedTag(active ? null : tag.name)}
                                                className={`shrink-0 flex items-center gap-1 rounded-full px-2.5 py-1 text-fs-xs font-semibold transition-colors ${active
                                                    ? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
                                                    : 'bg-surface-subtle text-text-tertiary hover:text-text-secondary'
                                                    }`}
                                            >
                                                <Tag className="h-2.5 w-2.5" />
                                                {tag.name}
                                                <span className="tabular-nums opacity-60">{tag.useCount}</span>
                                            </button>
                                        );
                                    })}
                                </>
                            )}
                        </div>

                        {spaces.length > 0 && (
                            <>
                                <div className="mx-1 h-4 w-px shrink-0 bg-stroke-divider" />
                                <button
                                    type="button"
                                    onClick={handleRefreshClassification}
                                    disabled={isRefreshingClassification}
                                    className="shrink-0 inline-flex items-center gap-1 rounded-full border border-stroke-control bg-surface-subtle px-3 py-1 text-fs-xs font-semibold text-text-secondary transition-colors hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-60"
                                >
                                    <RefreshCw className={`h-3 w-3 ${isRefreshingClassification ? 'animate-spin' : ''}`} />
                                    {t('repository.refresh_classification')}
                                </button>
                                <div className="relative shrink-0">
                                    <button
                                        type="button"
                                        onClick={() => setSpaceDropdownOpen((o) => !o)}
                                        className={`flex items-center gap-1 rounded-full border px-3 py-1 text-fs-xs font-semibold transition-colors focus:outline-none cursor-pointer ${spaceFilter
                                            ? 'border-accent-default bg-accent-default/10 text-accent-default'
                                            : 'border-stroke-control bg-surface-subtle text-text-secondary hover:text-text-primary'
                                            }`}
                                    >
                                        <span>{spaceFilter ? spaces.find((s) => s.id === spaceFilter)?.name ?? t('space_insight.select_space') : t('space_insight.select_space')}</span>
                                        <ChevronDown className={`h-3 w-3 transition-transform ${spaceDropdownOpen ? 'rotate-180' : ''}`} />
                                    </button>
                                    {spaceDropdownOpen && (
                                        <>
                                            <div className="fixed inset-0 z-40" onClick={() => setSpaceDropdownOpen(false)} />
                                            <div className="absolute right-0 top-full z-50 mt-1 min-w-[160px] rounded-xl border border-stroke-control bg-surface-flyout py-1 shadow-lg">
                                                <button
                                                    type="button"
                                                    onClick={() => { setSpaceFilter(null); setSpaceDropdownOpen(false); }}
                                                    className={`w-full px-3 py-1.5 text-left text-fs-xs font-semibold transition-colors hover:bg-surface-hover ${!spaceFilter ? 'text-accent-default' : 'text-text-secondary'}`}
                                                >
                                                    {t('space_insight.select_space')}
                                                </button>
                                                {spaces.map((s) => (
                                                    <button
                                                        key={s.id}
                                                        type="button"
                                                        onClick={() => { setSpaceFilter(s.id); setSpaceDropdownOpen(false); }}
                                                        className={`w-full px-3 py-1.5 text-left text-fs-xs font-semibold transition-colors hover:bg-surface-hover ${spaceFilter === s.id ? 'text-accent-default' : 'text-text-primary'}`}
                                                    >
                                                        {s.name}
                                                    </button>
                                                ))}
                                            </div>
                                        </>
                                    )}
                                </div>
                            </>
                        )}
                    </div>
                </div>
            </div>

            <div className="w-full px-6 py-5">
                {importProgressPanel}
                {isLoadingTimeline ? (
                    <div className="flex flex-col items-center justify-center py-20">
                        <div className="h-8 w-8 animate-spin rounded-full border-2 border-accent-default/20 border-t-accent-default" />
                        <p className="mt-3 text-fs-sm text-text-tertiary">{t('repository.loading')}</p>
                    </div>
                ) : visibleGroups.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-20">
                        <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-surface-subtle">
                            <Database className="h-6 w-6 text-text-tertiary" />
                        </div>
                        <p className="mt-4 text-fs-base font-semibold text-text-secondary">
                            {query ? t('repository.empty_no_match') : t('repository.empty_no_data')}
                        </p>
                        <p className="mt-1 text-fs-xs text-text-tertiary">
                            {query ? t('repository.empty_no_match_hint') : t('repository.empty_no_data_hint')}
                        </p>
                    </div>
                ) : (
                    <div className="grid grid-cols-[36px_minmax(0,1fr)] items-start gap-6 lg:grid-cols-[minmax(124px,148px)_minmax(0,1fr)]">
                        <aside className="sticky top-[160px]">
                            <div className="relative pl-6 pb-2 pt-32">
                                <div className="absolute bottom-1 left-2 top-1 w-px bg-stroke-divider" />
                                <ul className="space-y-3">
                                    {visibleGroups.map((group) => {
                                        const isActive = group.dateKey === selectedDateKey;
                                        return (
                                            <li
                                                key={group.dateKey}
                                                onClick={() => handleSelectDate(group.dateKey)}
                                                className="relative h-6 cursor-pointer lg:h-auto"
                                                title={`${formatDateLabel(group.dateKey, t)} ${formatDateSub(group.dateKey, t)}`.trim()}
                                            >
                                                <span
                                                    ref={(el) => {
                                                        markerRefs.current[group.dateKey] = el;
                                                    }}
                                                    className={`absolute left-[-19px] top-2.5 h-2.5 w-2.5 rounded-full border-2 ${isActive
                                                        ? 'border-accent-default bg-accent-default shadow-[0_0_0_3px_rgba(37,99,235,0.12)]'
                                                        : 'border-stroke-card bg-surface-base'
                                                        }`}
                                                />
                                                <div
                                                    className={`cursor-pointer rounded-lg px-1.5 py-1 transition-colors ${isActive
                                                        ? 'text-text-primary'
                                                        : 'text-text-secondary hover:text-text-primary'
                                                        } hidden lg:block`}
                                                >
                                                    <div className="text-fs-sm font-semibold">{formatDateLabel(group.dateKey, t)}</div>
                                                    <div className="mt-0.5 text-fs-xs text-text-tertiary">{formatDateSub(group.dateKey, t)}</div>
                                                </div>
                                            </li>
                                        );
                                    })}
                                </ul>
                            </div>
                        </aside>

                        <div className="min-w-0 space-y-4 pb-64">
                            {visibleGroups.map((group) => (
                                <section
                                    key={group.dateKey}
                                    ref={(el) => {
                                        sectionRefs.current[group.dateKey] = el;
                                    }}
                                    className="scroll-mt-[128px]"
                                >
                                    <div className="mb-3 flex items-baseline gap-2">
                                        <span className="text-fs-sm font-semibold text-text-primary">{formatDateLabel(group.dateKey, t)}</span>
                                        <span className="text-fs-xs text-text-tertiary">{formatDateSub(group.dateKey, t)}</span>
                                    </div>

                                    {(group.sourceItems.length > 0 || (group.dateKey === todayKey && shouldShowSourceAction)) && typeFilter !== 'note' && (
                                        <div className="mb-3">
                                            <div className="mb-2 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                                <FileText className="h-3 w-3" />
                                                {t('repository.section_sources')}
                                            </div>
                                            <div
                                                ref={(el) => {
                                                    firstRowRefs.current[group.dateKey] = el;
                                                }}
                                                className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5"
                                            >
                                                {group.sourceItems.map((item) => {
                                                    const { iconColor, iconBg, icon, mt } = getMediaConfig(item);
                                                    const embedded = getEmbeddedMedia(item);
                                                    const isVideoCard = mt === 'video' || embedded !== null || Boolean(item.thumbnail);
                                                    const bvid = extractBvidFromItem(item);
                                                    const thumbnailUrl = isVideoCard
                                                        ? (getVideoThumbnail(item) || (bvid ? bilibiliCovers[bvid] || null : null))
                                                        : null;
                                                    return (
                                                        <button
                                                            key={item.id}
                                                            type="button"
                                                            onClick={() => handleOpenSource(item)}
                                                            className="text-left group/card relative flex h-full min-h-[160px] flex-col overflow-hidden rounded-xl border border-stroke-card bg-surface-layer p-4 shadow-sm transition-all duration-200 hover:-translate-y-1 hover:shadow-[var(--shadow-card-hover)] cursor-pointer"
                                                        >
                                                            {isVideoCard ? (
                                                                <div className="flex flex-1 flex-col gap-3 w-full">
                                                                    <div className="relative overflow-hidden rounded-xl border border-stroke-card/70 bg-surface-subtle h-[96px]">
                                                                        {thumbnailUrl ? (
                                                                            <img
                                                                                src={thumbnailUrl}
                                                                                alt={titleOf(item.title)}
                                                                                className="h-full w-full object-cover"
                                                                                loading="lazy"
                                                                                referrerPolicy="no-referrer"
                                                                            />
                                                                        ) : (
                                                                            <div className="h-full w-full bg-gradient-to-br from-slate-700/70 via-slate-600/40 to-slate-800/80" />
                                                                        )}
                                                                        <div className="absolute inset-0 bg-black/20" />
                                                                        <div className="absolute left-2 top-2 inline-flex h-6 w-6 items-center justify-center rounded-full bg-black/45 text-white">
                                                                            <PlayCircle className="h-3.5 w-3.5" />
                                                                        </div>
                                                                    </div>
                                                                    <div className="min-w-0">
                                                                        <p className="line-clamp-2 text-[13.5px] font-medium text-text-primary leading-snug break-words">
                                                                            {titleOf(item.title)}
                                                                        </p>
                                                                        {item.contentPreview && (
                                                                            <p className="mt-1 line-clamp-3 text-[12px] leading-relaxed text-text-tertiary break-words">
                                                                                {item.contentPreview.replace(/\s+/g, ' ').trim()}
                                                                            </p>
                                                                        )}
                                                                    </div>
                                                                </div>
                                                            ) : (
                                                                <div className="flex flex-col flex-1 w-full gap-2">
                                                                    <div className="flex items-center gap-2.5">
                                                                        <div className={`shrink-0 flex h-8 w-8 items-center justify-center rounded-lg ${iconBg} ${iconColor}`}>
                                                                            <span className="scale-90">{icon}</span>
                                                                        </div>
                                                                        <p className="line-clamp-2 text-[13.5px] font-semibold text-text-primary leading-snug break-words flex-1 min-w-0">
                                                                            {titleOf(item.title)}
                                                                        </p>
                                                                    </div>
                                                                    {item.contentPreview && (
                                                                        <p className="line-clamp-4 text-[12px] leading-relaxed text-text-tertiary break-words flex-1">
                                                                            {item.contentPreview.replace(/\s+/g, ' ').trim()}
                                                                        </p>
                                                                    )}
                                                                </div>
                                                            )}
                                                            <div className="mt-4 flex flex-nowrap items-center gap-1.5 overflow-hidden w-full h-[24px] pr-6">
                                                                {(item.tags && item.tags.length > 0) ? item.tags.slice(0, 4).map((tag) => (
                                                                    <span key={tag} className="shrink-0 max-w-[100px] truncate rounded-md bg-surface-subtle px-2 py-0.5 text-[11px] font-medium text-text-secondary transition-colors group-hover/card:text-text-primary">#{tag}</span>
                                                                )) : null}
                                                            </div>
                                                            <div
                                                                onClick={(e) => {
                                                                    e.preventDefault();
                                                                    e.stopPropagation();
                                                                    handleDeleteSource(e as unknown as React.MouseEvent, item);
                                                                }}
                                                                role="button"
                                                                aria-label={t('repository.aria_delete_source', { title: titleOf(item.title) })}
                                                                className="absolute bottom-2 right-2 inline-flex h-6 w-6 items-center justify-center rounded-full border border-red-300/40 bg-red-500/10 text-red-500 opacity-0 transition-all hover:bg-red-500/20 hover:scale-110 group-hover/card:opacity-100 cursor-pointer z-10"
                                                            >
                                                                <Trash2 className="h-3 w-3" />
                                                            </div>
                                                        </button>
                                                    );
                                                })}
                                                {group.dateKey === todayKey && shouldShowSourceAction && (
                                                    <button
                                                        type="button"
                                                        aria-label={t('repository.import_file')}
                                                        onClick={handleAddTodayDocument}
                                                        className="group/card relative flex min-h-[160px] w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-accent-default/45 bg-accent-default/5 text-accent-default transition-all duration-200 hover:border-accent-default hover:bg-accent-default/10 hover:shadow-[var(--shadow-card-hover)]"
                                                    >
                                                        <span className="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-accent-default/25 bg-accent-default/12 transition-transform duration-200 group-hover/card:scale-105">
                                                            <Plus className="h-4 w-4" />
                                                        </span>
                                                        <span className="text-fs-sm font-semibold tracking-wide">{t('repository.import_file')}</span>
                                                        <span className="text-fs-xs text-accent-default/80">{t('repository.import_file_hint')}</span>
                                                    </button>
                                                )}
                                            </div>
                                        </div>
                                    )}

                                    {(group.noteItems.length > 0 || (group.dateKey === todayKey && shouldShowNoteAction)) && typeFilter !== 'source' && (
                                        <div className="mb-3">
                                            <div className="mb-2 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                                <NotebookPen className="h-3 w-3" />
                                                {t('repository.section_notes')}
                                            </div>
                                            <div
                                                ref={group.sourceItems.length === 0 ? (el) => {
                                                    firstRowRefs.current[group.dateKey] = el;
                                                } : undefined}
                                                className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5"
                                            >
                                                {group.noteItems.map((note) => {
                                                    return (
                                                        <button
                                                            key={note.id}
                                                            type="button"
                                                            onClick={() => handleOpenNote(note)}
                                                            className="group/card relative flex min-h-[120px] flex-col overflow-hidden rounded-xl border border-stroke-card bg-surface-layer shadow-[var(--shadow-card)] transition-all duration-200 hover:scale-[1.02] hover:shadow-[var(--shadow-card-hover)] active:scale-100 cursor-pointer"
                                                        >
                                                            <div className="flex flex-1 flex-col items-center justify-center gap-1.5 px-2.5">
                                                                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/10 text-amber-600 dark:bg-amber-500/20 dark:text-amber-400">
                                                                    <NotebookPen className="h-5 w-5" />
                                                                </div>
                                                                <p className="line-clamp-2 w-full text-center text-fs-sm font-medium text-text-primary">
                                                                    {titleOf(note.title)}
                                                                </p>
                                                            </div>
                                                            <div className="flex min-h-[30px] flex-wrap content-start gap-1 px-2 pb-2">
                                                                <span className="rounded-full bg-surface-subtle px-2 py-0.5 text-fs-xs text-text-tertiary">{t('repository.untagged')}</span>
                                                            </div>
                                                            <button
                                                                type="button"
                                                                onClick={(e) => handleDeleteNote(e, note)}
                                                                aria-label={t('repository.aria_delete_note', { title: titleOf(note.title) })}
                                                                className="absolute bottom-1.5 right-1.5 inline-flex h-6 w-6 items-center justify-center rounded-full border border-red-300/40 bg-red-500/10 text-red-500 opacity-0 transition-opacity hover:bg-red-500/20 group-hover/card:opacity-100"
                                                            >
                                                                <Trash2 className="h-3 w-3" />
                                                            </button>
                                                        </button>
                                                    );
                                                })}
                                                {group.dateKey === todayKey && shouldShowNoteAction && (
                                                    <button
                                                        type="button"
                                                        aria-label={t('repository.add_document')}
                                                        onClick={handleAddTodayNote}
                                                        className="group/card relative flex min-h-[120px] w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-amber-500/45 bg-amber-500/5 text-amber-600 transition-all duration-200 hover:border-amber-500 hover:bg-amber-500/10 hover:shadow-[var(--shadow-card-hover)] dark:text-amber-400"
                                                    >
                                                        <span className="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-amber-500/25 bg-amber-500/12 transition-transform duration-200 group-hover/card:scale-105">
                                                            <Plus className="h-4 w-4" />
                                                        </span>
                                                        <span className="text-fs-sm font-semibold tracking-wide">{t('repository.add_document')}</span>
                                                    </button>
                                                )}
                                            </div>
                                        </div>
                                    )}
                                </section>
                            ))}
                        </div>
                    </div>
                )}
            </div>

            {docPreview && (
                <div
                    className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 backdrop-blur-md sm:items-center"
                    style={{ animation: 'fadeInOverlay 0.2s ease' }}
                    onClick={() => { setDocPreview(null); setPreviewChunks([]); }}
                >
                    <style>{`
                        @keyframes fadeInOverlay { from { opacity: 0; } to { opacity: 1; } }
                        @keyframes slideUpDialog { from { opacity: 0; transform: translateY(24px) scale(0.98); } to { opacity: 1; transform: translateY(0) scale(1); } }
                    `}</style>
                    <div
                        className="flex h-[90vh] w-full max-w-[900px] flex-col overflow-hidden rounded-2xl border border-white/[0.08] bg-surface-flyout shadow-[0_32px_80px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.04)]"
                        style={{ animation: 'slideUpDialog 0.25s cubic-bezier(0.16,1,0.3,1)' }}
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div className="relative flex items-center gap-4 border-b border-white/[0.06] px-5 py-3.5">
                            <div className="absolute left-0 top-0 h-full w-[3px] rounded-l-2xl bg-gradient-to-b from-accent-default/80 via-accent-default/40 to-transparent" />

                            {docPreview.sourceItem && (() => {
                                const { iconColor, iconBg, icon } = getMediaConfig(docPreview.sourceItem);
                                return (
                                    <div className={`ml-2 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl ${iconBg} ${iconColor} shadow-sm`}>
                                        {icon}
                                    </div>
                                );
                            })()}

                            <div className="min-w-0 flex-1">
                                <h2 className="line-clamp-1 text-[15px] font-semibold leading-tight text-text-primary">{docPreview.title}</h2>
                            </div>

                            <div className="flex shrink-0 items-center gap-1.5">
                                {docPreview.sourceItem && (
                                    <button
                                        onClick={handleOpenOriginalFromPreview}
                                        className="flex items-center gap-1.5 rounded-lg border border-accent-default/30 bg-accent-default/10 px-3 py-1.5 text-[12px] font-semibold text-accent-default transition-all hover:border-accent-default/60 hover:bg-accent-default/20"
                                    >
                                        {t('repository.open_original')}
                                    </button>
                                )}
                                <button
                                    onClick={() => { setDocPreview(null); setPreviewChunks([]); }}
                                    className="flex h-8 w-8 items-center justify-center rounded-lg text-text-tertiary transition-all hover:bg-surface-subtle hover:text-text-primary"
                                >
                                    <X className="h-4 w-4" />
                                </button>
                            </div>
                        </div>

                        <div className="grid flex-1 overflow-hidden lg:grid-cols-[minmax(0,1fr)_280px]">

                            <section className="flex min-h-0 min-w-0 flex-1 flex-col">
                                <div className="min-h-0 flex-1 overflow-auto px-6 py-5">
                                    {embeddedMedia && (
                                        <div className="mb-5 overflow-hidden rounded-xl border border-white/[0.07] bg-black shadow-sm">
                                            <div className="relative w-full" style={{ paddingTop: '56.25%' }}>
                                                {embeddedMedia.kind === 'video' ? (
                                                    <video
                                                        className="absolute inset-0 h-full w-full"
                                                        controls
                                                        preload="metadata"
                                                        src={embeddedMedia.src}
                                                    />
                                                ) : (
                                                    <iframe
                                                        title={t('repository.embedded_source_player')}
                                                        src={embeddedMedia.src}
                                                        className="absolute inset-0 h-full w-full"
                                                        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
                                                        allowFullScreen
                                                    />
                                                )}
                                            </div>
                                        </div>
                                    )}


                                    {(() => {
                                        const raw = docPreview.content ?? '';
                                        const isMediaSource = embeddedMedia !== null || (docPreview.sourceItem?.mediaType === 'url') || (docPreview.sourceItem?.mediaType === 'video');
                                        const sourceUrl = docPreview.sourceItem ? inferOpenUrl(docPreview.sourceItem) : null;
                                        const isWebSource = Boolean(docPreview.sourceItem && docPreview.sourceItem.type === 'url' && sourceUrl);

                                        let display = raw;
                                        if (isMediaSource && raw) {
                                            const title = (docPreview.title ?? '').toLowerCase().replace(/\s+/g, ' ').trim();
                                            const lines = raw.split('\n');
                                            let startIdx = 0;
                                            for (let i = 0; i < Math.min(lines.length, 8); i++) {
                                                const line = lines[i].replace(/\s+/g, ' ').trim();
                                                const lineLower = line.toLowerCase();
                                                if (
                                                    line.length === 0 ||
                                                    (line.length > 2 && title.includes(lineLower)) ||
                                                    (lineLower.length > 2 && lineLower.includes(title.slice(0, 20))) ||
                                                    /^(YouTube|Bilibili)\b/i.test(line)
                                                ) {
                                                    startIdx = i + 1;
                                                } else {
                                                    break;
                                                }
                                            }
                                            display = lines.slice(startIdx).join('\n').trim();
                                        }

                                        if (!display && !isWebSource) return null;
                                        return (
                                            <div className="space-y-4">
                                                {display && (
                                                    <div className="rounded-xl border border-white/[0.07] bg-surface-subtle/40 px-5 py-4 shadow-sm">
                                                        <pre className="whitespace-pre-wrap break-words text-[13.5px] leading-[1.8] text-text-secondary">{display}</pre>
                                                    </div>
                                                )}
                                            </div>
                                        );
                                    })()}

                                    {docPreview.sourceId && (
                                        <div className="mt-5 overflow-hidden rounded-xl border border-white/[0.07] bg-surface-base shadow-sm">
                                            <div className="flex items-center justify-between border-b border-white/[0.05] bg-surface-layer/40 px-4 py-2.5">
                                                <span className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-widest text-text-secondary">
                                                    <Layers className="h-3 w-3" />
                                                    {t('repository.chunks')}
                                                </span>
                                                {previewChunks.length === 0 && !isLoadingChunks && (
                                                    <button
                                                        type="button"
                                                        onClick={async () => {
                                                            setIsLoadingChunks(true);
                                                            try {
                                                                const chunks = await invoke<CaptureDetail[]>('get_captures_detail', { sourceId: docPreview.sourceId });
                                                                setPreviewChunks(chunks);
                                                            } catch (err) {
                                                                console.error('Failed to load chunks:', err);
                                                            } finally {
                                                                setIsLoadingChunks(false);
                                                            }
                                                        }}
                                                        className="rounded-lg border border-accent-default/30 bg-accent-default/10 px-3 py-1 text-[11px] font-semibold text-accent-default transition-all hover:border-accent-default/60 hover:bg-accent-default/20"
                                                    >
                                                        {t('repository.load_chunks')}
                                                    </button>
                                                )}
                                            </div>
                                            <div className="p-4">
                                                {isLoadingChunks && (
                                                    <div className="flex items-center gap-2 py-3">
                                                        <div className="h-4 w-4 animate-spin rounded-full border-2 border-accent-default/20 border-t-accent-default" />
                                                        <span className="text-fs-xs text-text-tertiary">{t('repository.loading')}</span>
                                                    </div>
                                                )}
                                                {!isLoadingChunks && previewChunks.length === 0 && (
                                                    <div className="rounded-lg border border-dashed border-stroke-control px-3 py-4 text-center text-fs-xs text-text-tertiary">
                                                        {t('repository.no_chunks_loaded_yet')}
                                                    </div>
                                                )}
                                                {previewChunks.length > 0 && (
                                                    <div className="space-y-3">
                                                        {previewChunks.map((chunk, idx) => {
                                                            const isEditing = editingChunkId === chunk.id;
                                                            let parsedTags: string[] = [];
                                                            try { parsedTags = JSON.parse(chunk.tags) ?? []; } catch { /* noop */ }
                                                            const spaceName = spaces.find(s => s.id === chunk.spaceId)?.name;
                                                            return (
                                                                <div key={chunk.id} className="max-w-full overflow-hidden rounded-xl border border-white/[0.07] bg-surface-subtle/50 p-4 shadow-sm">
                                                                    <div className="mb-2.5 flex items-center gap-2">
                                                                        <span className="rounded-md bg-accent-default/12 px-2 py-0.5 text-[11px] font-bold tracking-wide text-accent-default">{`#${idx + 1}`}</span>
                                                                        {!isEditing && (
                                                                            <div className="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
                                                                                {spaceName && (
                                                                                    <span className="inline-flex items-center gap-1 rounded-full bg-accent-default/10 px-2 py-0.5 text-fs-xs text-accent-default">
                                                                                        <Layers size={10} />
                                                                                        {spaceName}
                                                                                    </span>
                                                                                )}
                                                                                {parsedTags.map(tag => (
                                                                                    <span key={tag} className="max-w-[120px] truncate rounded-full bg-surface-card px-2 py-0.5 text-fs-xs text-text-secondary">#{tag}</span>
                                                                                ))}
                                                                                {!spaceName && parsedTags.length === 0 && (
                                                                                    <span className="rounded-full bg-surface-card px-2 py-0.5 text-fs-xs text-text-tertiary">{t('repository.no_tags')}</span>
                                                                                )}
                                                                                <button
                                                                                    onClick={() => startEditChunk(chunk)}
                                                                                    className="flex items-center gap-1 rounded-md border border-stroke-control px-2.5 py-1 text-fs-xs text-text-tertiary transition-colors hover:bg-surface-card hover:text-text-primary"
                                                                                >
                                                                                    <Pencil size={11} />
                                                                                    {t('repository.chunk_edit_tags')}
                                                                                </button>
                                                                            </div>
                                                                        )}
                                                                        {isEditing && (
                                                                            <div className="ml-auto flex items-center gap-1">
                                                                                <button
                                                                                    onClick={cancelEditChunk}
                                                                                    className="rounded px-2 py-0.5 text-fs-xs text-text-tertiary hover:bg-surface-card transition-colors"
                                                                                >
                                                                                    {t('repository.chunk_cancel')}
                                                                                </button>
                                                                                <button
                                                                                    onClick={() => saveEditChunk(chunk.id)}
                                                                                    disabled={isSavingChunk}
                                                                                    className="flex items-center gap-1 rounded px-2 py-0.5 text-fs-xs bg-accent-default text-white hover:opacity-90 disabled:opacity-50 transition-opacity"
                                                                                >
                                                                                    <Check size={11} />
                                                                                    {t('repository.chunk_save')}
                                                                                </button>
                                                                            </div>
                                                                        )}
                                                                    </div>

                                                                    {isEditing && (
                                                                        <div className="mb-4 space-y-3 rounded-lg bg-surface-base p-3 border border-stroke-divider shadow-inner">
                                                                            <div>
                                                                                <label className="mb-1 block text-fs-xs font-semibold text-text-tertiary">{t('repository.chunk_edit_space')}</label>
                                                                                <div className="flex items-center gap-2">
                                                                                    <select
                                                                                        value={editSpaceId}
                                                                                        onChange={e => setEditSpaceId(e.target.value)}
                                                                                        className="flex-1 rounded-md border border-stroke-control bg-surface-layer px-2 py-1.5 text-fs-xs text-text-primary focus:border-accent-default focus:outline-none transition-colors"
                                                                                    >
                                                                                        <option value="">{t('repository.chunk_no_space')}</option>
                                                                                        {spaces.map(s => (
                                                                                            <option key={s.id} value={s.id}>{s.name}</option>
                                                                                        ))}
                                                                                    </select>
                                                                                    <button
                                                                                        type="button"
                                                                                        onClick={handleCreateNewSpace}
                                                                                        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-stroke-control bg-surface-layer text-text-tertiary transition-all hover:border-accent-default hover:bg-accent-default/10 hover:text-accent-default"
                                                                                        title={t('repository.create_space')}
                                                                                    >
                                                                                        <Plus size={14} />
                                                                                    </button>
                                                                                </div>
                                                                            </div>
                                                                            <div>
                                                                                <label className="mb-1 block text-fs-xs font-semibold text-text-tertiary">{t('repository.chunk_edit_tags')}</label>
                                                                                <div className="min-h-[36px] flex flex-wrap gap-1.5 rounded-md border border-stroke-control bg-surface-layer px-2.5 py-1.5 focus-within:border-accent-default transition-colors">
                                                                                    {editTags.map(tag => (
                                                                                        <span key={tag} className="inline-flex items-center gap-1 rounded-full bg-accent-default/10 px-2.5 py-0.5 text-fs-xs font-medium text-accent-default">
                                                                                            #{tag}
                                                                                            <button onClick={() => setEditTags(prev => prev.filter(t => t !== tag))} className="transition-colors hover:text-red-500">
                                                                                                <X size={10} />
                                                                                            </button>
                                                                                        </span>
                                                                                    ))}
                                                                                    <input
                                                                                        value={editTagInput}
                                                                                        onChange={e => setEditTagInput(e.target.value)}
                                                                                        onKeyDown={handleTagKeyDown}
                                                                                        placeholder={editTags.length === 0 ? t('repository.chunk_tag_placeholder') : ''}
                                                                                        className="min-w-[120px] flex-1 bg-transparent text-fs-xs text-text-primary outline-none placeholder:text-text-tertiary"
                                                                                    />
                                                                                </div>
                                                                            </div>
                                                                        </div>
                                                                    )}

                                                                    <pre className="w-full overflow-hidden whitespace-pre-wrap break-words text-[13.5px] leading-7 text-text-primary">{extractPlainText(chunk.cleanContent || '')}</pre>
                                                                </div>
                                                            );
                                                        })}
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </section>

                            <aside className="flex min-h-0 flex-col overflow-auto border-l border-white/[0.06] bg-surface-layer/20 p-3">
                                {!docPreview.sourceItem ? (
                                    <div className="rounded-lg border border-stroke-card bg-surface-base/50 p-3 text-[11px] text-text-tertiary">
                                        {t('repository.no_structured_source_metadata')}
                                    </div>
                                ) : (
                                    <div className="space-y-3">
                                        {/* 擷取時間 */}
                                        {docPreview.sourceItem.capturedAt && (
                                            <div className="overflow-hidden rounded-lg border border-white/[0.07] bg-surface-base/50 px-3 py-2">
                                                <div className="mb-0.5 text-[10px] text-text-tertiary">{t('repository.source_meta_captured')}</div>
                                                <div className="text-[11px] font-medium text-text-primary">
                                                    {new Date(docPreview.sourceItem.capturedAt).toLocaleString('zh-TW', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}
                                                </div>
                                            </div>
                                        )}

                                        {/* 標籤 */}
                                        {docPreview.sourceItem.tags && docPreview.sourceItem.tags.length > 0 && (
                                            <div className="overflow-hidden rounded-lg border border-white/[0.07] bg-surface-base/50 px-3 py-2">
                                                <div className="mb-1.5 text-[10px] text-text-tertiary">{t('repository.source_meta_tags')}</div>
                                                <div className="flex flex-wrap gap-1">
                                                    {docPreview.sourceItem.tags.map(tag => (
                                                        <span key={tag} className="rounded-full bg-surface-card px-2 py-0.5 text-[11px] text-text-secondary">#{tag}</span>
                                                    ))}
                                                </div>
                                            </div>
                                        )}

                                        {/* URL */}
                                        {docPreview.sourceItem.url && (
                                            <div className="overflow-hidden rounded-lg border border-white/[0.07] bg-surface-base/50 px-3 py-2">
                                                <div className="mb-0.5 text-[10px] text-text-tertiary">{t('repository.source_meta_url')}</div>
                                                <button
                                                    type="button"
                                                    onClick={handleOpenOriginalFromPreview}
                                                    className="break-all text-left text-[11px] leading-4 text-blue-400 hover:text-blue-300 transition-colors"
                                                >
                                                    {docPreview.sourceItem.url}
                                                </button>
                                            </div>
                                        )}

                                        {/* 本機路徑 */}
                                        {(docPreview.sourceItem.localDocPath || docPreview.sourceItem.filePath) && (
                                            <div className="overflow-hidden rounded-lg border border-white/[0.07] bg-surface-base/50 px-3 py-2">
                                                <div className="mb-0.5 text-[10px] text-text-tertiary">{t('repository.source_meta_file')}</div>
                                                <div className="break-all font-mono text-[10px] leading-4 text-text-secondary">{docPreview.sourceItem.localDocPath || docPreview.sourceItem.filePath}</div>
                                            </div>
                                        )}
                                    </div>
                                )}
                            </aside>
                        </div>
                    </div>
                </div>
            )}

            {imagePreview && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" onClick={() => setImagePreview(null)}>
                    <div className="relative w-full max-w-6xl" onClick={(e) => e.stopPropagation()}>
                        <button onClick={() => setImagePreview(null)} className="absolute -top-10 right-0 rounded-lg p-1.5 text-white/60 transition-colors hover:text-white">
                            <X className="h-5 w-5" />
                        </button>
                        <img src={imagePreview.src} alt={imagePreview.title} className="max-h-[85vh] w-full rounded-xl object-contain" />
                        <div className="mt-3 text-center text-fs-sm text-white/70">{imagePreview.title}</div>
                    </div>
                </div>
            )}
        </div>
    );
};
