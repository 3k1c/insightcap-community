import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Search, FileText, ExternalLink, PlayCircle, ImageIcon, NotebookPen, X, Database, Trash2, Plus, Tag, Layers } from 'lucide-react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';
import { useKnowledgeStore, type TimelineSourceItem, type CaptureDetail } from '../stores/knowledgeStore';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { loadFiles, loadContent, deleteFile, type NoteFile } from '../lib/noteStore';
import { tauriCmd } from '../lib/tauri';
import { useTagStore } from '../stores/tagStore';
import { useT } from '../hooks/useT';

/* ── helpers ── */

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

    const cleaned = t
        .replace(/\s*-\s*(modified|已修改)$/i, '')
        .replace(/\s*-\s*(visual studio code|vscode)$/i, '')
        .replace(/\s*-\s*insightcap$/i, '')
        .replace(/\s*-\s*(visual studio code|vscode)\s*-\s*(modified|已修改)$/i, '')
        .replace(/\s*-\s*insightcap\s*-\s*(visual studio code|vscode)(\s*-\s*(modified|已修改))?$/i, '')
        .replace(/\s{2,}/g, ' ')
        .trim();

    return cleaned;
}

function inferPath(item: TimelineSourceItem): string | null {
    return item.localDocPath || item.filePath || null;
}

/** 從 TipTap/ProseMirror JSON 中遞迴提取純文字 */
function extractPlainText(raw: string): string {
    try {
        const doc = JSON.parse(raw);
        if (doc && doc.type === 'doc' && Array.isArray(doc.content)) {
            return walkNodes(doc.content).trim();
        }
    } catch {
        // not JSON — return as-is
    }
    return raw;
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
        // block-level nodes get a trailing newline
        if (['paragraph', 'heading', 'blockquote', 'codeBlock', 'listItem', 'bulletList', 'orderedList'].includes(n.type as string)) {
            out += '\n';
        }
    }
    return out;
}

const mediaColor: Record<string, string> = {
    url:   'text-blue-500',
    video: 'text-purple-500',
    image: 'text-emerald-500',
    pdf:   'text-red-400',
    text:  'text-text-secondary',
};

const mediaBg: Record<string, string> = {
    url:   'bg-gradient-to-br from-blue-50 to-blue-100 dark:from-blue-500/15 dark:to-blue-600/10',
    video: 'bg-gradient-to-br from-purple-50 to-purple-100 dark:from-purple-500/15 dark:to-purple-600/10',
    image: 'bg-gradient-to-br from-emerald-50 to-emerald-100 dark:from-emerald-500/15 dark:to-emerald-600/10',
    pdf:   'bg-gradient-to-br from-red-50 to-red-100 dark:from-red-400/15 dark:to-red-500/10',
    text:  'bg-gradient-to-br from-gray-50 to-gray-100 dark:from-white/5 dark:to-white/10',
};

/* ── types ── */

interface DayGroup {
    dateKey: string;
    sourceItems: TimelineSourceItem[];
    noteItems: NoteFile[];
}

interface PreviewDoc {
    title: string;
    content: string;
    sourceId?: string;
}

/* ── component ── */

export const RepositoryPage: React.FC = () => {
    const t = useT();
    const [query, setQuery] = useState('');
    const [notes, setNotes] = useState<NoteFile[]>([]);
    const [selectedDateKey, setSelectedDateKey] = useState<string | null>(null);
    const [docPreview, setDocPreview] = useState<PreviewDoc | null>(null);
    const [imagePreview, setImagePreview] = useState<{ src: string; title: string } | null>(null);
    const [previewChunks, setPreviewChunks] = useState<CaptureDetail[]>([]);
    const [isLoadingChunks, setIsLoadingChunks] = useState(false);
    const [typeFilter, setTypeFilter] = useState<'all' | 'source' | 'note'>('all');
    const [selectedTag, setSelectedTag] = useState<string | null>(null);
    const scrollerRef = useRef<HTMLDivElement | null>(null);
    const sectionRefs = useRef<Record<string, HTMLElement | null>>({});
    const firstRowRefs = useRef<Record<string, HTMLDivElement | null>>({});
    const markerRefs = useRef<Record<string, HTMLSpanElement | null>>({});

    const timelineSources = useKnowledgeStore((s) => s.timelineSources);
    const isLoadingTimeline = useKnowledgeStore((s) => s.isLoadingTimeline);
    const loadTimeline = useKnowledgeStore((s) => s.loadTimeline);
    const deleteSource = useKnowledgeStore((s) => s.deleteSource);
    const titleOf = (value: string | undefined | null) => safeTitle(value) || t('repository.untitled');

    const recentTags = useTagStore((s) => s.recentTags);
    const loadRecentTags = useTagStore((s) => s.loadRecentTags);

    useEffect(() => {
        loadTimeline();
        loadRecentTags();
        try { setNotes(loadFiles()); } catch { /* ignore */ }
    }, [loadTimeline, loadRecentTags]);

    /* ── stats ── */

    const totalChunks = useMemo(
        () => (Array.isArray(timelineSources) ? timelineSources : []).reduce((sum, s) => sum + (s.captureCount || 0), 0),
        [timelineSources],
    );

    const topTags = useMemo(() => {
        const sorted = [...recentTags].sort((a, b) => b.recentCount - a.recentCount || b.useCount - a.useCount);
        return sorted.slice(0, 5);
    }, [recentTags]);

    /* ── filtering ── */

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

    /* ── date grouping ── */

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

    /* ── handlers ── */

    const handleOpenSource = async (item: TimelineSourceItem) => {
        const path = inferPath(item);

        if (item.mediaType === 'image' && path) {
            setImagePreview({ src: convertFileSrc(path), title: titleOf(item.title) });
            return;
        }
        if (item.mediaType === 'url' && item.url) {
            window.open(item.url, '_blank', 'noopener,noreferrer');
            return;
        }
        if (item.mediaType === 'video') {
            const target = item.url || (path ? convertFileSrc(path) : null);
            if (target) window.open(target, '_blank', 'noopener,noreferrer');
            return;
        }
        if (path) {
            try {
                const content = await tauriCmd.openDocument(path);
                setDocPreview({ title: titleOf(item.title), content: extractPlainText(content), sourceId: item.id });
            } catch {
                await tauriCmd.openFileInSystem(path);
            }
            return;
        }
        if (item.url) {
            window.open(item.url, '_blank', 'noopener,noreferrer');
            return;
        }
        setDocPreview({ title: titleOf(item.title), content: extractPlainText(item.contentPreview || t('repository.no_content')), sourceId: item.id });
    };

    const handleOpenNote = (note: NoteFile) => {
        const raw = loadContent(note.id) || t('repository.note_empty');
        setDocPreview({ title: titleOf(note.title), content: extractPlainText(raw), sourceId: undefined });
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
                    extensions: ['txt', 'md', 'doc', 'docx', 'xlsx', 'csv', 'pptx', 'pdf', 'py', 'js', 'ts', 'jsx', 'tsx', 'swift', 'rs', 'go', 'java', 'cpp', 'c', 'h', 'rb', 'php', 'html'],
                }],
            });

            const paths = selected === null ? [] : Array.isArray(selected) ? selected : [selected];
            if (paths.length === 0) return;

            const results = await Promise.allSettled(paths.map((path) => tauriCmd.ingestFile(path)));
            const successCount = results.filter((r) => r.status === 'fulfilled').length;
            const failCount = results.length - successCount;

            if (successCount > 0) {
                toast.success(t('repository.toast_import_success', { count: String(successCount) }));
            }
            if (failCount > 0) {
                toast.error(t('repository.toast_import_failed', { count: String(failCount) }));
            }

            await loadTimeline();
        } catch (error) {
            console.error('Failed to import files:', error);
            toast.error(t('repository.toast_open_dialog_failed'));
        }
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

    /* ── render ── */

    const totalCount = filteredSources.length + filteredNotes.length;
    const todayKey = new Date().toISOString().slice(0, 10);

    /* tag filter: load source IDs that match the selected tag */
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

    /* apply type + tag filter to dayGroups for rendering */
    const visibleGroups = useMemo(() => {
        let groups = dayGroups;

        // type filter
        if (typeFilter !== 'all') {
            groups = groups
                .map((g) => ({
                    ...g,
                    sourceItems: typeFilter === 'source' ? g.sourceItems : [],
                    noteItems: typeFilter === 'note' ? g.noteItems : [],
                }))
                .filter((g) => g.sourceItems.length > 0 || g.noteItems.length > 0 || (typeFilter !== 'note' && g.dateKey === todayKey));
        }

        // tag filter (only affects sources — notes don't have tags)
        if (tagSourceIds) {
            groups = groups
                .map((g) => ({
                    ...g,
                    sourceItems: g.sourceItems.filter((s) => tagSourceIds.has(s.id)),
                }))
                .filter((g) => g.sourceItems.length > 0 || g.noteItems.length > 0 || g.dateKey === todayKey);
        }

        return groups;
    }, [dayGroups, typeFilter, todayKey, tagSourceIds]);

    return (
        <div ref={scrollerRef} className="flex-1 overflow-auto bg-surface-base">
            {/* ── Header ── */}
            <div className="sticky top-0 z-10 border-b border-stroke-divider bg-surface-base/80 px-6 pb-4 pt-6 backdrop-blur-md">
                <div className="w-full">
                    <div className="mb-4 flex items-center gap-3">
                        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-accent-light2">
                            <Database className="h-4.5 w-4.5 text-accent-default" />
                        </div>
                        <h1 className="font-ui text-fs-xl font-bold text-text-primary">{t('repository.page_title')}</h1>

                        {/* ── stat cards ── */}
                        <div className="ml-auto flex items-center gap-2">
                            {[
                                { label: t('repository.stat_sources'), count: filteredSources.length, icon: <FileText className="h-3.5 w-3.5" />, color: 'text-blue-500 bg-blue-500/10' },
                                { label: t('repository.stat_notes'), count: filteredNotes.length, icon: <NotebookPen className="h-3.5 w-3.5" />, color: 'text-amber-500 bg-amber-500/10' },
                                { label: t('repository.stat_chunks'), count: totalChunks, icon: <Layers className="h-3.5 w-3.5" />, color: 'text-purple-500 bg-purple-500/10' },
                                { label: t('repository.stat_tags'), count: recentTags.length, icon: <Tag className="h-3.5 w-3.5" />, color: 'text-emerald-500 bg-emerald-500/10' },
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

                    {/* ── filter row ── */}
                    <div className="mt-3 flex items-center gap-2">
                        {(['all', 'source', 'note'] as const).map((f) => {
                            const label = f === 'all' ? t('repository.filter_all') : f === 'source' ? t('repository.filter_source') : t('repository.filter_note');
                            const active = typeFilter === f;
                            return (
                                <button
                                    key={f}
                                    type="button"
                                    onClick={() => setTypeFilter(f)}
                                    className={`rounded-full px-3 py-1 text-fs-xs font-semibold transition-colors ${
                                        active
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
                                <div className="mx-1 h-4 w-px bg-stroke-divider" />
                                {topTags.map((tag) => {
                                    const active = selectedTag === tag.name;
                                    return (
                                        <button
                                            key={tag.id}
                                            type="button"
                                            onClick={() => setSelectedTag(active ? null : tag.name)}
                                            className={`flex items-center gap-1 rounded-full px-2.5 py-1 text-fs-xs font-semibold transition-colors ${
                                                active
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
                </div>
            </div>

            {/* ── Content ── */}
            <div className="w-full px-6 py-5">
                {isLoadingTimeline ? (
                    <div className="flex flex-col items-center justify-center py-20">
                        <div className="h-8 w-8 animate-spin rounded-full border-2 border-accent-default/20 border-t-accent-default" />
                        <p className="mt-3 text-fs-sm text-text-tertiary">{t('repository.loading')}</p>
                    </div>
                ) : totalCount === 0 ? (
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
                                                    className={`absolute left-[-19px] top-2.5 h-2.5 w-2.5 rounded-full border-2 ${
                                                        isActive
                                                            ? 'border-accent-default bg-accent-default shadow-[0_0_0_3px_rgba(37,99,235,0.12)]'
                                                            : 'border-stroke-card bg-surface-base'
                                                    }`}
                                                />
                                                <div
                                                    className={`cursor-pointer rounded-lg px-1.5 py-1 transition-colors ${
                                                        isActive
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

                                    {(group.sourceItems.length > 0 || group.dateKey === todayKey) && (
                                        <div className="mb-3">
                                            <div className="mb-2 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                                <FileText className="h-3 w-3" />
                                                {t('repository.section_sources')}
                                            </div>
                                            <div
                                                ref={(el) => {
                                                    firstRowRefs.current[group.dateKey] = el;
                                                }}
                                                className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-4 xl:grid-cols-4"
                                            >
                                                {group.sourceItems.map((item) => {
                                                    const mt = item.mediaType || 'text';
                                                    const iconColor = mediaColor[mt] || mediaColor.text;
                                                    const iconBg = mediaBg[mt] || mediaBg.text;
                                                    const icon =
                                                        mt === 'url' ? <ExternalLink className="h-6 w-6" /> :
                                                        mt === 'video' ? <PlayCircle className="h-6 w-6" /> :
                                                        mt === 'image' ? <ImageIcon className="h-6 w-6" /> :
                                                        <FileText className="h-6 w-6" />;
                                                    return (
                                                        <button
                                                            key={item.id}
                                                            type="button"
                                                            onClick={() => handleOpenSource(item)}
                                                            className="group/card relative flex min-h-[120px] flex-col overflow-hidden rounded-xl border border-stroke-card bg-surface-layer shadow-[var(--shadow-card)] transition-all duration-200 hover:scale-[1.02] hover:shadow-[var(--shadow-card-hover)] active:scale-100 cursor-pointer"
                                                        >
                                                            <div className="flex flex-1 flex-col items-center justify-center gap-1.5 px-2.5">
                                                                <div className={`flex h-10 w-10 items-center justify-center rounded-xl ${iconBg} ${iconColor}`}>
                                                                    <span className="scale-90">{icon}</span>
                                                                </div>
                                                                <p className="line-clamp-2 w-full text-center text-fs-sm font-medium text-text-primary">
                                                                    {titleOf(item.title)}
                                                                </p>
                                                            </div>
                                                            <div className="flex min-h-[30px] flex-wrap content-start gap-1 px-2 pb-2">
                                                                {(item.tags && item.tags.length > 0) ? item.tags.slice(0, 3).map((tag) => (
                                                                    <span key={tag} className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-fs-xs text-emerald-600 dark:text-emerald-400">#{tag}</span>
                                                                )) : (
                                                                    <span className="rounded-full bg-surface-subtle px-2 py-0.5 text-fs-xs text-text-tertiary">{t('repository.untagged')}</span>
                                                                )}
                                                            </div>
                                                            <button
                                                                type="button"
                                                                onClick={(e) => handleDeleteSource(e, item)}
                                                                aria-label={t('repository.aria_delete_source', { title: titleOf(item.title) })}
                                                                className="absolute bottom-1.5 right-1.5 inline-flex h-6 w-6 items-center justify-center rounded-full border border-red-300/40 bg-red-500/10 text-red-500 opacity-0 transition-opacity hover:bg-red-500/20 group-hover/card:opacity-100"
                                                            >
                                                                <Trash2 className="h-3 w-3" />
                                                            </button>
                                                        </button>
                                                    );
                                                })}
                                                {group.dateKey === todayKey && (
                                                    <button
                                                        type="button"
                                                        onClick={handleAddTodayDocument}
                                                        className="group/card relative flex min-h-[120px] w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-accent-default/45 bg-[linear-gradient(145deg,rgba(37,99,235,0.08),rgba(37,99,235,0.03))] text-accent-default transition-all duration-200 hover:border-accent-default hover:bg-[linear-gradient(145deg,rgba(37,99,235,0.14),rgba(37,99,235,0.06))] hover:shadow-[var(--shadow-card-hover)]"
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

                                    {group.noteItems.length > 0 && (
                                        <div className="mb-3">
                                            <div className="mb-2 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                                <NotebookPen className="h-3 w-3" />
                                                {t('repository.section_notes')}
                                            </div>
                                            <div
                                                ref={group.sourceItems.length === 0 ? (el) => {
                                                    firstRowRefs.current[group.dateKey] = el;
                                                } : undefined}
                                                className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-4 xl:grid-cols-4"
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
                                                                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-amber-50 to-amber-100 text-amber-500 dark:from-amber-500/15 dark:to-amber-600/10">
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
                                            </div>
                                        </div>
                                    )}
                                </section>
                            ))}
                        </div>
                    </div>
                )}
            </div>

            {/* ── Document Preview Modal ── */}
            {docPreview && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm" onClick={() => { setDocPreview(null); setPreviewChunks([]); }}>
                    <div
                        className="flex h-[80vh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl border border-stroke-card bg-surface-flyout shadow-[var(--shadow-dialog)]"
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div className="flex items-center justify-between border-b border-stroke-divider px-5 py-3">
                            <span className="text-fs-base font-semibold text-text-primary">{docPreview.title}</span>
                            <button onClick={() => { setDocPreview(null); setPreviewChunks([]); }} className="rounded-lg p-1.5 text-text-tertiary transition-colors hover:bg-surface-subtle hover:text-text-primary">
                                <X className="h-4 w-4" />
                            </button>
                        </div>
                        <div className="flex-1 overflow-auto">
                            <pre className="whitespace-pre-wrap p-5 text-fs-base text-text-primary">{docPreview.content}</pre>

                            {/* ── Chunks ── */}
                            {docPreview.sourceId && (
                                <div className="border-t border-stroke-divider px-5 py-4">
                                    <div className="mb-3 flex items-center justify-between">
                                        <span className="text-fs-sm font-semibold text-text-secondary">{t('repository.chunks')}</span>
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
                                                className="rounded-lg px-2.5 py-1 text-fs-xs font-semibold text-accent-default transition-colors hover:bg-accent-default/10"
                                            >
                                                {t('repository.load_chunks')}
                                            </button>
                                        )}
                                    </div>
                                    {isLoadingChunks && (
                                        <div className="flex items-center gap-2 py-3">
                                            <div className="h-4 w-4 animate-spin rounded-full border-2 border-accent-default/20 border-t-accent-default" />
                                            <span className="text-fs-xs text-text-tertiary">{t('repository.loading')}</span>
                                        </div>
                                    )}
                                    {previewChunks.length > 0 && (
                                        <div className="space-y-2">
                                            {previewChunks.map((chunk, idx) => (
                                                <div key={chunk.id} className="rounded-xl border border-stroke-card bg-surface-subtle p-3">
                                                    <div className="mb-1.5 flex items-center gap-2">
                                                        <span className="rounded-md bg-accent-default/10 px-1.5 py-0.5 text-fs-xs font-semibold text-accent-default">#{idx + 1}</span>
                                                        <span className="rounded-md bg-surface-base px-1.5 py-0.5 text-fs-xs text-text-tertiary">{chunk.type}</span>
                                                        <span className="rounded-md bg-surface-base px-1.5 py-0.5 text-fs-xs text-text-tertiary">{chunk.status}</span>
                                                    </div>
                                                    <pre className="whitespace-pre-wrap text-fs-sm text-text-primary">{extractPlainText(chunk.cleanContent || '')}</pre>
                                                </div>
                                            ))}
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}

            {/* ── Image Preview Modal ── */}
            {imagePreview && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm" onClick={() => setImagePreview(null)}>
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