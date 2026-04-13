import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { X, Check, XCircle } from 'lucide-react';
import { useT } from '../../hooks/useT';

interface PendingPattern {
    id: string;
    knowledgeType: string;
    content: string;
    tags: string;
    confidence: number;
    createdAt: string;
}

export function PatternPromotionToast() {
    const t = useT();
    const [patterns, setPatterns] = useState<PendingPattern[]>([]);
    const [current, setCurrent] = useState<PendingPattern | null>(null);

    const loadPending = useCallback(async () => {
        try {
            const list = await invoke<PendingPattern[]>('get_pending_patterns');
            setPatterns(list);
            if (list.length > 0 && !current) {
                setCurrent(list[0]);
            }
        } catch (e) {
            console.error('Failed to load pending patterns:', e);
        }
    }, [current]);

    useEffect(() => {
        loadPending();

        const unlisten = listen<number>('pattern-promoted', () => {
            loadPending();
        });

        return () => { unlisten.then(fn => fn()); };
    }, [loadPending]);

    async function handleConfirm(accept: boolean) {
        if (!current) return;
        try {
            await invoke('confirm_pattern', {
                captureId: current.id,
                accept,
            });
            advanceToNext();
        } catch (e) {
            console.error('Failed to confirm pattern:', e);
        }
    }

    function advanceToNext() {
        const remaining = patterns.filter(p => p.id !== current?.id);
        setPatterns(remaining);
        setCurrent(remaining.length > 0 ? remaining[0] : null);
    }

    if (!current) return null;

    const parsedTags: string[] = (() => {
        try { return JSON.parse(current.tags); } catch { return []; }
    })();

    return (
        <div className="fixed bottom-4 left-4 z-50 w-96 rounded-xl border border-stroke-divider bg-surface-base shadow-flyout animate-in slide-in-from-bottom-4 duration-300">
            {/* Header */}
            <div className="flex items-center justify-between px-4 pt-3 pb-2">
                <span className="text-fs-sm font-semibold text-text-primary">
                    {t('pattern_promotion.title')}
                </span>
                <button
                    onClick={() => handleConfirm(false)}
                    className="rounded-md p-1 text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle"
                >
                    <X size={14} />
                </button>
            </div>

            <div className="px-4 pb-4 space-y-3">
                <p className="text-fs-xs text-text-tertiary">
                    {t('pattern_promotion.desc')}
                </p>

                {/* Pattern content preview */}
                <div className="rounded-lg bg-surface-subtle p-3 space-y-2">
                    <div className="text-fs-sm text-text-primary leading-relaxed line-clamp-4">
                        {current.content}
                    </div>

                    {parsedTags.length > 0 && (
                        <div className="flex flex-wrap gap-1">
                            {parsedTags.map(tag => (
                                <span
                                    key={tag}
                                    className="rounded-full bg-accent-default/10 px-2 py-0.5 text-fs-xs text-accent-default"
                                >
                                    #{tag}
                                </span>
                            ))}
                        </div>
                    )}

                    <div className="text-fs-xs text-text-tertiary">
                        {t('pattern_promotion.confidence', { value: Math.round(current.confidence * 100) })}
                    </div>
                </div>

                {/* Remaining count */}
                {patterns.length > 1 && (
                    <p className="text-fs-xs text-text-tertiary">
                        {t('pattern_promotion.remaining', { count: patterns.length - 1 })}
                    </p>
                )}

                {/* Actions */}
                <div className="flex gap-2">
                    <button
                        onClick={() => handleConfirm(false)}
                        className="flex-1 flex items-center justify-center gap-1.5 rounded-lg border border-stroke-control px-3 py-1.5 text-fs-xs text-text-secondary hover:bg-surface-subtle"
                    >
                        <XCircle size={14} />
                        {t('pattern_promotion.reject')}
                    </button>
                    <button
                        onClick={() => handleConfirm(true)}
                        className="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-accent-default px-3 py-1.5 text-fs-xs text-text-on-accent hover:bg-accent-dark1"
                    >
                        <Check size={14} />
                        {t('pattern_promotion.accept')}
                    </button>
                </div>
            </div>
        </div>
    );
}
