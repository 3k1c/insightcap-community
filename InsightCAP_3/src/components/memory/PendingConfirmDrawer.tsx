import React, { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { X, Check, XCircle, ChevronRight } from 'lucide-react';
import { useT } from '../../hooks/useT';

// ─── 型別 ────────────────────────────────────────────────────────────────────

interface PendingChunk {
    id: string;
    knowledgeType: string;
    content: string;
    tags: string;
    confidence: number;
    createdAt: string;
}

const TYPE_META: Record<string, { symbol: string; colorClass: string; labelKey: string }> = {
    data:    { symbol: '●', colorClass: 'text-blue-500',  labelKey: 'pending_drawer.type_data' },
    pattern: { symbol: '◆', colorClass: 'text-amber-500', labelKey: 'pending_drawer.type_pattern' },
    log:     { symbol: '▲', colorClass: 'text-red-500',   labelKey: 'pending_drawer.type_log' },
};

// ─── Hook：取得 pending count（供 indicator 使用）────────────────────────────

export function usePendingConfirmCount() {
    const [count, setCount] = useState(0);

    const refresh = useCallback(async () => {
        try {
            const list = await invoke<PendingChunk[]>('get_pending_memory_chunks');
            setCount(list.filter(c => c.knowledgeType !== 'pattern').length);
        } catch {
            setCount(0);
        }
    }, []);

    useEffect(() => {
        refresh();
        const unlisten = listen('summary-completed', refresh);
        return () => { unlisten.then(fn => fn()); };
    }, [refresh]);

    return count;
}

// ─── Drawer ──────────────────────────────────────────────────────────────────

interface Props {
    open: boolean;
    onClose: () => void;
    onCountChange?: (count: number) => void;
}

export function PendingConfirmDrawer({ open, onClose, onCountChange }: Props) {
    const t = useT();
    const [chunks, setChunks] = useState<PendingChunk[]>([]);
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [isSubmitting, setIsSubmitting] = useState(false);

    const loadPending = useCallback(async () => {
        try {
            const list = await invoke<PendingChunk[]>('get_pending_memory_chunks');
            const nonPattern = list.filter(c => c.knowledgeType !== 'pattern');
            setChunks(nonPattern);
            // 預設全選
            setSelected(new Set(nonPattern.map(c => c.id)));
            onCountChange?.(nonPattern.length);
        } catch {
            setChunks([]);
            onCountChange?.(0);
        }
    }, [onCountChange]);

    useEffect(() => {
        if (open) loadPending();
    }, [open, loadPending]);

    useEffect(() => {
        const unlisten = listen('summary-completed', loadPending);
        return () => { unlisten.then(fn => fn()); };
    }, [loadPending]);

    const allSelected = chunks.length > 0 && selected.size === chunks.length;

    function toggleSelectAll() {
        if (allSelected) {
            setSelected(new Set());
        } else {
            setSelected(new Set(chunks.map(c => c.id)));
        }
    }

    function toggleOne(id: string) {
        setSelected(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id); else next.add(id);
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
        } finally {
            setIsSubmitting(false);
        }
    }

    if (!open) return null;

    return (
        <>
            {/* Backdrop */}
            <div
                className="fixed inset-0 z-40 bg-surface-base"
                onClick={onClose}
            />

            {/* Drawer */}
            <div className="fixed right-0 top-0 bottom-0 z-50 w-96 flex flex-col bg-surface-card border-l border-stroke-divider shadow-flyout animate-in slide-in-from-right duration-200">
                {/* Header */}
                <div className="flex items-center justify-between px-5 py-4 border-b border-stroke-divider shrink-0">
                    <span className="text-fs-base font-semibold text-text-primary">
                        {t('pending_drawer.title')}
                        {chunks.length > 0 && (
                            <span className="ml-2 rounded-full bg-red-500 px-2 py-0.5 text-fs-xs text-white font-medium">
                                {chunks.length}
                            </span>
                        )}
                    </span>
                    <button
                        onClick={onClose}
                        className="rounded-md p-1.5 text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle"
                    >
                        <X size={16} />
                    </button>
                </div>

                {/* Content */}
                <div className="flex-1 overflow-y-auto">
                    {chunks.length === 0 ? (
                        <div className="flex items-center justify-center h-full text-fs-sm text-text-tertiary">
                            {t('pending_drawer.empty')}
                        </div>
                    ) : (
                        <ul className="divide-y divide-stroke-divider">
                            {chunks.map(chunk => {
                                const meta = TYPE_META[chunk.knowledgeType] ?? TYPE_META.data;
                                const isChecked = selected.has(chunk.id);
                                return (
                                    <li
                                        key={chunk.id}
                                        className={`flex items-start gap-3 px-5 py-4 cursor-pointer transition-colors ${isChecked ? 'bg-surface-subtle' : 'hover:bg-surface-subtle/50'}`}
                                        onClick={() => toggleOne(chunk.id)}
                                    >
                                        {/* Checkbox */}
                                        <div className={`mt-0.5 shrink-0 w-4 h-4 rounded border flex items-center justify-center transition-colors ${isChecked ? 'bg-accent-default border-accent-default' : 'border-stroke-control'}`}>
                                            {isChecked && <Check size={10} className="text-white" strokeWidth={3} />}
                                        </div>

                                        {/* Body */}
                                        <div className="flex-1 min-w-0">
                                            <div className="flex items-center gap-1.5 mb-1">
                                                <span className={`text-fs-xs ${meta.colorClass}`}>{meta.symbol}</span>
                                                <span className="text-fs-xs text-text-tertiary">{t(meta.labelKey)}</span>
                                                <span className="text-fs-xs text-text-tertiary ml-auto">
                                                    {t('pending_drawer.confidence', { value: Math.round(chunk.confidence * 100) })}
                                                </span>
                                            </div>
                                            <p className="text-fs-sm text-text-primary leading-relaxed line-clamp-3">
                                                {chunk.content}
                                            </p>
                                        </div>
                                    </li>
                                );
                            })}
                        </ul>
                    )}
                </div>

                {/* Footer toolbar */}
                {chunks.length > 0 && (
                    <div className="shrink-0 border-t border-stroke-divider px-5 py-3 space-y-2">
                        {/* 全選 toggle */}
                        <button
                            onClick={toggleSelectAll}
                            className="text-fs-xs text-text-secondary hover:text-text-primary flex items-center gap-1"
                        >
                            <ChevronRight size={12} className={`transition-transform ${allSelected ? 'rotate-90' : ''}`} />
                            {allSelected ? t('pending_drawer.deselect_all') : t('pending_drawer.select_all')}
                        </button>

                        {/* 批量操作 */}
                        <div className="flex gap-2">
                            <button
                                onClick={() => handleBatch(false)}
                                disabled={selected.size === 0 || isSubmitting}
                                className="flex-1 flex items-center justify-center gap-1.5 rounded-lg border border-stroke-control px-3 py-2 text-fs-xs text-text-secondary hover:bg-surface-subtle disabled:opacity-40 transition-colors"
                            >
                                <XCircle size={13} />
                                {t('pending_drawer.reject_selected', { count: selected.size })}
                            </button>
                            <button
                                onClick={() => handleBatch(true)}
                                disabled={selected.size === 0 || isSubmitting}
                                className="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-accent-default px-3 py-2 text-fs-xs text-white hover:bg-accent-dark1 disabled:opacity-40 transition-colors"
                            >
                                <Check size={13} />
                                {t('pending_drawer.accept_selected', { count: selected.size })}
                            </button>
                        </div>
                    </div>
                )}
            </div>
        </>
    );
}
