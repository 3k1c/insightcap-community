import React, { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { X, Check, XCircle } from 'lucide-react';
import { useT } from '../../hooks/useT';

interface PendingChunk {
    id: string;
    knowledgeType: string;
    content: string;
    tags: string;
    confidence: number;
    createdAt: string;
}

const TYPE_BADGE: Record<string, { label: string; colorClass: string }> = {
    data: { label: '●', colorClass: 'text-blue-500' },
    pattern: { label: '◆', colorClass: 'text-amber-500' },
    log: { label: '▲', colorClass: 'text-red-500' },
};

export function MemoryConfirmToast() {
    const t = useT();
    const [chunks, setChunks] = useState<PendingChunk[]>([]);
    const [current, setCurrent] = useState<PendingChunk | null>(null);

    const loadPending = useCallback(async () => {
        try {
            const list = await invoke<PendingChunk[]>('get_pending_memory_chunks');
            // 排除 pattern 類型（由 PatternPromotionToast 處理）
            const nonPattern = list.filter(c => c.knowledgeType !== 'pattern');
            setChunks(nonPattern);
            if (nonPattern.length > 0 && !current) {
                setCurrent(nonPattern[0]);
            }
        } catch (e) {
            console.error('Failed to load pending memory chunks:', e);
        }
    }, [current]);

    useEffect(() => {
        loadPending();

        const unlisten = listen('summary-completed', () => {
            loadPending();
        });

        return () => { unlisten.then(fn => fn()); };
    }, [loadPending]);

    async function handleConfirm(accept: boolean) {
        if (!current) return;
        try {
            await invoke('confirm_memory_chunk', {
                chunkId: current.id,
                accept,
            });
            advanceToNext();
        } catch (e) {
            console.error('Failed to confirm memory chunk:', e);
        }
    }

    function advanceToNext() {
        const remaining = chunks.filter(c => c.id !== current?.id);
        setChunks(remaining);
        setCurrent(remaining.length > 0 ? remaining[0] : null);
    }

    if (!current) return null;

    const badge = TYPE_BADGE[current.knowledgeType] || TYPE_BADGE.data;

    return (
        <div className="fixed bottom-20 right-4 z-50 w-80 rounded-xl border border-stroke-divider bg-surface-card shadow-flyout animate-in slide-in-from-bottom-4 duration-300">
            {/* Header */}
            <div className="flex items-center justify-between px-4 pt-3 pb-2">
                <div className="flex items-center gap-2">
                    <span className={`text-fs-base ${badge.colorClass}`}>{badge.label}</span>
                    <span className="text-fs-sm font-semibold text-text-primary">
                        {t('memory_confirm.title')}
                    </span>
                </div>
                <button
                    onClick={() => handleConfirm(false)}
                    className="rounded-md p-1 text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle"
                >
                    <X size={14} />
                </button>
            </div>

            <div className="px-4 pb-4 space-y-3">
                <p className="text-fs-xs text-text-tertiary">
                    {t('memory_confirm.desc', { type: current.knowledgeType })}
                </p>

                {/* Content preview */}
                <div className="rounded-lg bg-surface-subtle p-3">
                    <div className="text-fs-sm text-text-primary leading-relaxed line-clamp-3">
                        {current.content}
                    </div>
                    <div className="mt-1.5 text-fs-xs text-text-tertiary">
                        {t('memory_confirm.confidence', { value: Math.round(current.confidence * 100) })}
                    </div>
                </div>

                {/* Remaining count */}
                {chunks.length > 1 && (
                    <p className="text-fs-xs text-text-tertiary">
                        {t('memory_confirm.remaining', { count: chunks.length - 1 })}
                    </p>
                )}

                {/* Actions */}
                <div className="flex gap-2">
                    <button
                        onClick={() => handleConfirm(false)}
                        className="flex-1 flex items-center justify-center gap-1.5 rounded-lg border border-stroke-control px-3 py-1.5 text-fs-xs text-text-secondary hover:bg-surface-subtle"
                    >
                        <XCircle size={14} />
                        {t('memory_confirm.reject')}
                    </button>
                    <button
                        onClick={() => handleConfirm(true)}
                        className="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-accent-default px-3 py-1.5 text-fs-xs text-text-on-accent hover:bg-accent-dark1"
                    >
                        <Check size={14} />
                        {t('memory_confirm.accept')}
                    </button>
                </div>
            </div>
        </div>
    );
}
