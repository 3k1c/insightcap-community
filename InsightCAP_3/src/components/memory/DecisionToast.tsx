import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { useT } from '../../hooks/useT';

interface Decision {
    id: string;
    variableDesc: string;
    chosenOption: string;
    triggerAt: string;
    createdAt: string;
}

type Rating = 'good' | 'ok' | 'bad' | 'critical';

const RATING_OPTIONS: { value: Rating; colorClass: string }[] = [
    { value: 'good', colorClass: 'bg-green-500/10 text-green-600 border-green-500/20 hover:bg-green-500/20' },
    { value: 'ok', colorClass: 'bg-blue-500/10 text-blue-600 border-blue-500/20 hover:bg-blue-500/20' },
    { value: 'bad', colorClass: 'bg-amber-500/10 text-amber-600 border-amber-500/20 hover:bg-amber-500/20' },
    { value: 'critical', colorClass: 'bg-red-500/10 text-red-600 border-red-500/20 hover:bg-red-500/20' },
];

export function DecisionReviewToast() {
    const t = useT();
    const [decisions, setDecisions] = useState<Decision[]>([]);
    const [current, setCurrent] = useState<Decision | null>(null);
    const [selectedRating, setSelectedRating] = useState<Rating | null>(null);
    const [note, setNote] = useState('');
    const [submitting, setSubmitting] = useState(false);

    useEffect(() => {
        loadDueDecisions();
        // 每 5 分鐘檢查一次
        const interval = setInterval(loadDueDecisions, 5 * 60 * 1000);
        return () => clearInterval(interval);
    }, []);

    async function loadDueDecisions() {
        try {
            const list = await invoke<Decision[]>('get_due_decisions');
            setDecisions(list);
            if (list.length > 0 && !current) {
                setCurrent(list[0]);
            }
        } catch (e) {
            console.error('Failed to load due decisions:', e);
        }
    }

    async function handleSubmit() {
        if (!current || !selectedRating) return;
        setSubmitting(true);
        try {
            await invoke('report_decision_outcome', {
                decisionId: current.id,
                outcomeRating: selectedRating,
                outcomeNote: note || null,
            });
            advanceToNext();
        } catch (e) {
            console.error('Failed to report decision:', e);
        } finally {
            setSubmitting(false);
        }
    }

    async function handleDismiss() {
        if (!current) return;
        try {
            await invoke('dismiss_decision', { decisionId: current.id });
            advanceToNext();
        } catch (e) {
            console.error('Failed to dismiss decision:', e);
        }
    }

    function advanceToNext() {
        const remaining = decisions.filter(d => d.id !== current?.id);
        setDecisions(remaining);
        setCurrent(remaining.length > 0 ? remaining[0] : null);
        setSelectedRating(null);
        setNote('');
    }

    if (!current) return null;

    const daysSince = Math.floor(
        (Date.now() - new Date(current.createdAt).getTime()) / (1000 * 60 * 60 * 24)
    );

    return (
        <div className="fixed bottom-4 right-4 z-50 w-80 rounded-xl border border-stroke-divider bg-surface-card shadow-flyout animate-in slide-in-from-bottom-4 duration-300">
            {/* Header */}
            <div className="flex items-center justify-between px-4 pt-3 pb-2">
                <span className="text-fs-sm font-semibold text-text-primary">
                    {t('decision.review_title')}
                </span>
                <button
                    onClick={handleDismiss}
                    className="rounded-md p-1 text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle"
                >
                    <X size={14} />
                </button>
            </div>

            <div className="px-4 pb-4 space-y-3">
                <p className="text-fs-xs text-text-tertiary">
                    {t('decision.review_desc', { days: daysSince })}
                </p>

                {/* Decision info */}
                <div className="rounded-lg bg-surface-subtle p-3 space-y-1.5">
                    <div className="text-fs-xs text-text-tertiary">{t('decision.variable')}</div>
                    <div className="text-fs-sm text-text-primary">{current.variableDesc}</div>
                    <div className="text-fs-xs text-text-tertiary mt-1">{t('decision.chosen')}</div>
                    <div className="text-fs-sm text-text-primary">{current.chosenOption}</div>
                </div>

                {/* Rating buttons */}
                <div className="grid grid-cols-2 gap-2">
                    {RATING_OPTIONS.map(opt => (
                        <button
                            key={opt.value}
                            onClick={() => setSelectedRating(opt.value)}
                            className={`rounded-lg border px-3 py-1.5 text-fs-xs font-medium transition-colors ${
                                selectedRating === opt.value
                                    ? opt.colorClass + ' ring-1 ring-current'
                                    : 'border-stroke-control bg-surface-layer text-text-secondary hover:bg-surface-subtle'
                            }`}
                        >
                            {t(`decision.rating_${opt.value}`)}
                        </button>
                    ))}
                </div>

                {/* Note */}
                {selectedRating && (
                    <textarea
                        value={note}
                        onChange={e => setNote(e.target.value)}
                        placeholder={t('decision.note_placeholder')}
                        rows={2}
                        className="w-full rounded-md border border-stroke-control bg-surface-layer px-3 py-2 text-fs-xs text-text-primary placeholder:text-text-tertiary focus:outline-none focus:border-accent-default"
                    />
                )}

                {/* Actions */}
                <div className="flex gap-2">
                    <button
                        onClick={handleDismiss}
                        className="flex-1 rounded-lg border border-stroke-control px-3 py-1.5 text-fs-xs text-text-secondary hover:bg-surface-subtle"
                    >
                        {t('decision.dismiss')}
                    </button>
                    <button
                        onClick={handleSubmit}
                        disabled={!selectedRating || submitting}
                        className="flex-1 rounded-lg bg-accent-default px-3 py-1.5 text-fs-xs text-text-on-accent hover:bg-accent-dark1 disabled:opacity-50"
                    >
                        {t('decision.submit')}
                    </button>
                </div>
            </div>
        </div>
    );
}
