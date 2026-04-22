import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { Check, ChevronDown, Clock3, X, XCircle } from 'lucide-react';
import { useT } from '../../hooks/useT';

interface PendingChunk {
    id: string;
    knowledgeType: string;
    content: string;
    tags: string;
    confidence: number;
    createdAt: string;
}

interface Props {
    open: boolean;
    onClose: () => void;
    onCountChange?: (count: number) => void;
}

type ReviewTab = 'memory' | 'pattern';

const TYPE_META: Record<string, { labelKey: string; chipClass: string; accentClass: string }> = {
    data: {
        labelKey: 'pending_drawer.type_data',
        chipClass: 'bg-sky-100 text-sky-800',
        accentClass: 'bg-accent-default',
    },
    pattern: {
        labelKey: 'pending_drawer.type_pattern',
        chipClass: 'bg-amber-100 text-amber-800',
        accentClass: 'bg-amber-500',
    },
    log: {
        labelKey: 'pending_drawer.type_log',
        chipClass: 'bg-rose-100 text-rose-800',
        accentClass: 'bg-accent-default',
    },
};

function hasRenderableContent(value: string | null | undefined) {
    return Boolean(value && value.trim().length > 0);
}

function isPattern(chunk: PendingChunk) {
    return chunk.knowledgeType === 'pattern';
}

function parseTags(tags: string) {
    let parsed: string[] = [];
    try {
        parsed = JSON.parse(tags);
    } catch {
        parsed = [tags];
    }
    return parsed.filter(tag => tag && tag !== 'untagged');
}

function toDisplayDate(ts: string) {
    const date = new Date(ts);
    if (Number.isNaN(date.getTime())) return '';
    return date.toLocaleString();
}

function normalizePendingList(list: PendingChunk[]) {
    const byId = new Map<string, PendingChunk>();
    for (const item of list) {
        if (hasRenderableContent(item.content)) {
            byId.set(item.id, item);
        }
    }
    return Array.from(byId.values());
}

export function usePendingConfirmCount() {
    const [count, setCount] = useState(0);

    const refresh = useCallback(async () => {
        try {
            const list = await invoke<PendingChunk[]>('get_pending_memory_chunks');
            setCount(normalizePendingList(list).length);
        } catch {
            setCount(0);
        }
    }, []);

    useEffect(() => {
        refresh();
        const u1 = listen('summary-completed', refresh);
        const u2 = listen('pattern-promoted', refresh);
        const u3 = listen('memory-chunks-updated', refresh);
        return () => {
            u1.then(fn => fn());
            u2.then(fn => fn());
            u3.then(fn => fn());
        };
    }, [refresh]);

    return count;
}

export function PendingConfirmDrawer({ open, onClose, onCountChange }: Props) {
    const t = useT();
    const [chunks, setChunks] = useState<PendingChunk[]>([]);
    const [activeTab, setActiveTab] = useState<ReviewTab>('memory');
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [expanded, setExpanded] = useState<Set<string>>(new Set());
    const [isSubmitting, setIsSubmitting] = useState(false);

    const memoryChunks = useMemo(() => chunks.filter(chunk => !isPattern(chunk)), [chunks]);
    const patternChunks = useMemo(() => chunks.filter(isPattern), [chunks]);
    const visibleChunks = activeTab === 'memory' ? memoryChunks : patternChunks;

    const loadPending = useCallback(async () => {
        try {
            const list = await invoke<PendingChunk[]>('get_pending_memory_chunks');
            const pending = normalizePendingList(list);
            const nextMemory = pending.filter(chunk => !isPattern(chunk));
            const nextPattern = pending.filter(isPattern);
            setChunks(pending);
            setSelected(new Set());
            setExpanded(new Set());
            setActiveTab(prev => {
                if (prev === 'memory' && nextMemory.length === 0 && nextPattern.length > 0) return 'pattern';
                if (prev === 'pattern' && nextPattern.length === 0 && nextMemory.length > 0) return 'memory';
                return prev;
            });
            onCountChange?.(pending.length);
        } catch {
            setChunks([]);
            onCountChange?.(0);
        }
    }, [onCountChange]);

    useEffect(() => {
        if (open) loadPending();
    }, [open, loadPending]);

    useEffect(() => {
        const u1 = listen('summary-completed', loadPending);
        const u2 = listen('pattern-promoted', loadPending);
        const u3 = listen('memory-chunks-updated', loadPending);
        return () => {
            u1.then(fn => fn());
            u2.then(fn => fn());
            u3.then(fn => fn());
        };
    }, [loadPending]);

    useEffect(() => {
        setSelected(new Set());
        setExpanded(new Set());
    }, [activeTab]);

    const allSelected = visibleChunks.length > 0 && selected.size === visibleChunks.length;

    const selectedRate = useMemo(() => {
        if (visibleChunks.length === 0) return 0;
        return Math.round((selected.size / visibleChunks.length) * 100);
    }, [selected.size, visibleChunks.length]);

    function toggleSelectAll() {
        if (allSelected) {
            setSelected(new Set());
            return;
        }
        setSelected(new Set(visibleChunks.map(chunk => chunk.id)));
    }

    function toggleOne(id: string) {
        setSelected(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    }

    async function handleBatch(accept: boolean) {
        if (selected.size === 0) return;
        setIsSubmitting(true);
        try {
            await invoke('batch_confirm_memory_chunks', {
                chunkIds: Array.from(selected),
                accept,
            });
            await loadPending();
            await emit('memory-chunks-updated');
        } finally {
            setIsSubmitting(false);
        }
    }

    if (!open) return null;

    const activeMeta = activeTab === 'pattern' ? TYPE_META.pattern : TYPE_META.data;

    return (
        <>
            <div className="fixed inset-0 z-40 bg-black/45 backdrop-blur-[1px]" onClick={onClose} />

            <section className="fixed right-0 top-0 bottom-0 z-50 w-[420px] max-w-[95vw] flex flex-col border-l border-stroke-divider bg-surface-flyout shadow-flyout animate-in slide-in-from-right duration-200">
                <header className="shrink-0 border-b border-stroke-divider px-5 pt-5 pb-4">
                    <div className="flex items-start justify-between gap-3">
                        <div>
                            <h2 className="text-fs-lg font-semibold text-text-primary">{t('pending_drawer.title')}</h2>
                            <p className="mt-1 text-fs-xs text-text-tertiary">
                                {selected.size} / {visibleChunks.length}
                            </p>
                        </div>
                        <button
                            onClick={onClose}
                            className="rounded-md p-1.5 text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle"
                            aria-label={t('pending_drawer.close_aria')}
                        >
                            <X size={16} />
                        </button>
                    </div>

                    <div className="mt-3 grid grid-cols-2 gap-1 rounded-lg bg-surface-base p-1">
                        <button
                            onClick={() => setActiveTab('memory')}
                            className={`rounded-md px-3 py-1.5 text-fs-xs transition-colors ${activeTab === 'memory' ? 'bg-accent-default text-white' : 'text-text-secondary hover:bg-surface-subtle'}`}
                        >
                            {t('pending_drawer.tab_memory')} {memoryChunks.length}
                        </button>
                        <button
                            onClick={() => setActiveTab('pattern')}
                            className={`rounded-md px-3 py-1.5 text-fs-xs transition-colors ${activeTab === 'pattern' ? 'bg-amber-500 text-white' : 'text-text-secondary hover:bg-surface-subtle'}`}
                        >
                            {t('pending_drawer.tab_pattern')} {patternChunks.length}
                        </button>
                    </div>

                    {visibleChunks.length > 0 && (
                        <div className="mt-3 rounded-lg border border-stroke-control bg-surface-base px-3 py-2">
                            <div className="flex items-center justify-between text-fs-xs text-text-secondary">
                                <span>{t('pending_drawer.selected_label')}</span>
                                <span>{selectedRate}%</span>
                            </div>
                            <div className="mt-2 h-1.5 rounded-full bg-surface-subtle">
                                <div
                                    className={`h-full rounded-full transition-all duration-200 ${activeMeta.accentClass}`}
                                    style={{ width: `${selectedRate}%` }}
                                />
                            </div>
                        </div>
                    )}
                </header>

                <div className="flex-1 overflow-y-auto px-4 py-4">
                    {visibleChunks.length === 0 ? (
                        <div className="h-full rounded-xl border border-dashed border-stroke-control bg-surface-base flex items-center justify-center text-fs-sm text-text-tertiary">
                            {t('pending_drawer.empty')}
                        </div>
                    ) : (
                        <ul className="space-y-3">
                            {visibleChunks.map(chunk => {
                                const meta = TYPE_META[chunk.knowledgeType] ?? TYPE_META.data;
                                const tags = parseTags(chunk.tags);
                                const isChecked = selected.has(chunk.id);
                                const isExpanded = expanded.has(chunk.id);

                                return (
                                    <li
                                        key={chunk.id}
                                        className={`rounded-xl border p-3 transition-colors ${isChecked ? 'border-accent-default/40 bg-accent-light2/20' : 'border-stroke-control bg-surface-base hover:bg-surface-subtle'}`}
                                    >
                                        <div className="flex items-start gap-3">
                                            <button
                                                onClick={() => toggleOne(chunk.id)}
                                                className={`mt-0.5 shrink-0 w-4 h-4 rounded border flex items-center justify-center transition-colors ${isChecked ? `${meta.accentClass} border-transparent` : 'border-stroke-control'}`}
                                                aria-label={`toggle-${chunk.id}`}
                                            >
                                                {isChecked && <Check size={10} className="text-white" strokeWidth={3} />}
                                            </button>

                                            <div className="flex-1 min-w-0">
                                                <div className="flex items-center gap-2">
                                                    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${meta.chipClass}`}>
                                                        {t(meta.labelKey)}
                                                    </span>
                                                    <span className="ml-auto text-fs-xs text-text-tertiary">
                                                        {t('pending_drawer.confidence', { value: Math.round(chunk.confidence * 100) })}
                                                    </span>
                                                </div>

                                                <p className={`mt-2 text-fs-sm leading-relaxed text-text-primary ${isExpanded ? '' : 'line-clamp-3'}`}>
                                                    {chunk.content.trim()}
                                                </p>

                                                <div className="mt-2 flex items-center justify-between">
                                                    <span className="inline-flex items-center gap-1 text-[11px] text-text-tertiary">
                                                        <Clock3 size={12} />
                                                        {toDisplayDate(chunk.createdAt)}
                                                    </span>
                                                    <button
                                                        onClick={() =>
                                                            setExpanded(prev => {
                                                                const next = new Set(prev);
                                                                if (next.has(chunk.id)) next.delete(chunk.id);
                                                                else next.add(chunk.id);
                                                                return next;
                                                            })
                                                        }
                                                        className="inline-flex items-center gap-1 text-[11px] text-text-secondary hover:text-text-primary"
                                                    >
                                                        <ChevronDown size={12} className={`transition-transform ${isExpanded ? 'rotate-180' : ''}`} />
                                                        {isExpanded ? 'Collapse' : 'Expand'}
                                                    </button>
                                                </div>

                                                {isExpanded && tags.length > 0 && (
                                                    <div className="mt-2 flex flex-wrap gap-1.5">
                                                        {tags.map((tag, i) => (
                                                            <span key={`${chunk.id}-${i}`} className="rounded bg-surface-layer px-1.5 py-0.5 text-[11px] text-text-secondary">
                                                                {tag}
                                                            </span>
                                                        ))}
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    </li>
                                );
                            })}
                        </ul>
                    )}
                </div>

                {visibleChunks.length > 0 && (
                    <footer className="shrink-0 border-t border-stroke-divider bg-surface-base px-4 py-3">
                        <div className="mb-2 flex items-center justify-between text-fs-xs">
                            <button onClick={toggleSelectAll} className="text-text-secondary hover:text-text-primary">
                                {allSelected ? t('pending_drawer.deselect_all') : t('pending_drawer.select_all')}
                            </button>
                            <span className="text-text-tertiary">{selected.size}</span>
                        </div>
                        <div className="grid grid-cols-2 gap-2">
                            <button
                                onClick={() => handleBatch(false)}
                                disabled={selected.size === 0 || isSubmitting}
                                className="inline-flex items-center justify-center gap-1.5 rounded-lg border border-stroke-control px-3 py-2 text-fs-xs text-text-secondary hover:bg-surface-subtle disabled:opacity-40"
                            >
                                <XCircle size={13} />
                                {t('pending_drawer.reject_selected', { count: selected.size })}
                            </button>
                            <button
                                onClick={() => handleBatch(true)}
                                disabled={selected.size === 0 || isSubmitting}
                                className={`inline-flex items-center justify-center gap-1.5 rounded-lg px-3 py-2 text-fs-xs text-white disabled:opacity-40 ${activeTab === 'pattern' ? 'bg-amber-500 hover:bg-amber-600' : 'bg-accent-default hover:bg-accent-dark1'}`}
                            >
                                <Check size={13} />
                                {t('pending_drawer.accept_selected', { count: selected.size })}
                            </button>
                        </div>
                    </footer>
                )}
            </section>
        </>
    );
}
