import React, { useEffect, useState, useMemo, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useT } from '../hooks/useT';
import { Calendar, Clock, Check, X, Bell, Plus, Search, Tag, FileText, Layers } from 'lucide-react';
import { toast } from 'sonner';

interface Reminder {
    id: string;
    title: string;
    description: string | null;
    eventType: string;
    eventDate: string | null;
    eventTime: string | null;
    status: string;
    pendingConfirm: number;
}

function formatDateLabel(key: string, t: (key: string) => string): string {
    const today = new Date().toISOString().slice(0, 10);
    if (key === today) return t('repository.today');
    const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
    if (key === yesterday) return t('repository.yesterday');
    const [, m, d] = key.split('-');
    if (!m || !d) return t('repository.unknown') || 'Unknown';
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
    return isNaN(d.getTime()) ? '' : `${t('repository.week_prefix') || ''}${days[d.getDay()]}`;
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

export const SchedulePage: React.FC = () => {
    const t = useT();
    const [activeReminders, setActiveReminders] = useState<Reminder[]>([]);
    const [pendingReminders, setPendingReminders] = useState<Reminder[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [selectedDateKey, setSelectedDateKey] = useState<string | null>(null);
    const [query, setQuery] = useState('');
    const [typeFilter, setTypeFilter] = useState<'all' | 'meeting' | 'deliverable' | 'event' | 'appointment'>('all');

    const scrollerRef = useRef<HTMLDivElement | null>(null);
    const sectionRefs = useRef<Record<string, HTMLElement | null>>({});
    const markerRefs = useRef<Record<string, HTMLSpanElement | null>>({});

    const loadReminders = async () => {
        setIsLoading(true);
        try {
            const active = await invoke<Reminder[]>('get_active_reminders');
            const pending = await invoke<Reminder[]>('get_pending_reminders');
            setActiveReminders(active);
            setPendingReminders(pending);
        } catch (error) {
            console.error('Failed to load reminders:', error);
            toast.error(t('common.error'));
        } finally {
            setIsLoading(false);
        }
    };

    useEffect(() => {
        loadReminders();
    }, []);

    const handleConfirm = async (id: string, accept: boolean) => {
        try {
            await invoke('confirm_reminder', { reminderId: id, accept });
            loadReminders();
        } catch (error) {
            console.error('Confirm failed:', error);
            toast.error(t('common.error'));
        }
    };

    const handleUpdateStatus = async (id: string, status: string) => {
        try {
            await invoke('update_reminder_status', { reminderId: id, status });
            loadReminders();
        } catch (error) {
            console.error('Update status failed:', error);
            toast.error(t('common.error'));
        }
    };

    const todayKey = new Date().toISOString().slice(0, 10);

    const filteredActive = useMemo(() => {
        return activeReminders.filter(r => {
            const matchesQuery = !query ||
                r.title.toLowerCase().includes(query.toLowerCase()) ||
                (r.description?.toLowerCase().includes(query.toLowerCase()) ?? false);
            const matchesType = typeFilter === 'all' || r.eventType === typeFilter;
            return matchesQuery && matchesType;
        });
    }, [activeReminders, query, typeFilter]);

    const filteredPending = useMemo(() => {
        return pendingReminders.filter(r => {
            const matchesQuery = !query ||
                r.title.toLowerCase().includes(query.toLowerCase()) ||
                (r.description?.toLowerCase().includes(query.toLowerCase()) ?? false);
            const matchesType = typeFilter === 'all' || r.eventType === typeFilter;
            return matchesQuery && matchesType;
        });
    }, [pendingReminders, query, typeFilter]);

    const dayGroups = useMemo(() => {
        const groups: Record<string, { pending: Reminder[], active: Reminder[] }> = {};

        const addToGroup = (r: Reminder, isPending: boolean) => {
            const key = safeDateKey(r.eventDate);
            if (!groups[key]) groups[key] = { pending: [], active: [] };
            if (isPending) groups[key].pending.push(r);
            else groups[key].active.push(r);
        };

        filteredPending.forEach(r => addToGroup(r, true));
        filteredActive.forEach(r => addToGroup(r, false));

        if (!groups[todayKey] && !query) {
            groups[todayKey] = { pending: [], active: [] };
        }

        return Object.entries(groups)
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([dateKey, items]) => ({ dateKey, ...items }));
    }, [filteredActive, filteredPending, todayKey, query]);

    const handleSelectDate = (dateKey: string) => {
        setSelectedDateKey(dateKey);
        const scroller = scrollerRef.current;
        const marker = markerRefs.current[dateKey];
        const section = sectionRefs.current[dateKey];

        if (!scroller || !section) return;

        if (!marker) {
            section.scrollIntoView({ behavior: 'smooth', block: 'start' });
            return;
        }

        const markerRect = marker.getBoundingClientRect();
        const targetRect = section.getBoundingClientRect();
        const markerCenterY = markerRect.top + markerRect.height / 2;
        const targetCenterY = targetRect.top + targetRect.height / 2;
        const nextTop = scroller.scrollTop + (targetCenterY - markerCenterY) - 50;

        scroller.scrollTo({ top: Math.max(0, nextTop), behavior: 'smooth' });
    };

    const renderReminderCard = (r: Reminder, pending: boolean) => (
        <div key={r.id} className="bg-surface-layer border border-stroke-divider rounded-xl p-3 flex flex-col gap-2 shadow-sm transition-all duration-200 hover:-translate-y-1 hover:shadow-[var(--shadow-card-hover)]">
            <div className="flex justify-between items-start gap-4">
                <div className="min-w-0 flex-1">
                    <h3 className="text-[13.5px] font-semibold text-text-primary leading-snug mb-1 flex items-center gap-2">
                        <span className="truncate">{r.title}</span>
                        <span className="shrink-0 px-2 py-0.5 rounded-full bg-surface-subtle text-text-tertiary text-[10px] uppercase font-bold tracking-wider">
                            {r.eventType}
                        </span>
                    </h3>
                    {r.description && <p className="text-[12px] leading-relaxed text-text-tertiary line-clamp-2">{r.description}</p>}
                </div>
            </div>

            {(r.eventDate || r.eventTime) && (
                <div className="flex items-center gap-3 text-xs text-text-tertiary">
                    {r.eventDate && (
                        <span className="flex items-center gap-1.5 bg-surface-subtle px-2 py-1 rounded-md">
                            <Calendar className="w-3.5 h-3.5" />
                            {r.eventDate}
                        </span>
                    )}
                    {r.eventTime && (
                        <span className="flex items-center gap-1.5 bg-surface-subtle px-2 py-1 rounded-md">
                            <Clock className="w-3.5 h-3.5" />
                            {r.eventTime}
                        </span>
                    )}
                </div>
            )}

            <div className="flex justify-end gap-2 mt-auto pt-2 border-t border-stroke-divider">
                {pending ? (
                    <>
                        <button
                            onClick={() => handleConfirm(r.id, false)}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-text-tertiary hover:text-red-500 hover:bg-surface-subtle rounded-md transition-colors"
                        >
                            <X className="w-3.5 h-3.5" />
                            {t('common.cancel')}
                        </button>
                        <button
                            onClick={() => handleConfirm(r.id, true)}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-white bg-accent-default hover:bg-accent-hover rounded-md transition-colors"
                        >
                            <Check className="w-3.5 h-3.5" />
                            {t('common.confirm')}
                        </button>
                    </>
                ) : (
                    <>
                        <button
                            onClick={() => handleUpdateStatus(r.id, 'dismissed')}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-text-tertiary hover:text-red-500 hover:bg-surface-subtle rounded-md transition-colors"
                        >
                            <X className="w-3.5 h-3.5" />
                            {t('common.cancel')}
                        </button>
                        <button
                            onClick={() => handleUpdateStatus(r.id, 'completed')}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-emerald-600 hover:text-emerald-700 hover:bg-emerald-50 rounded-md transition-colors font-medium border border-emerald-200"
                        >
                            <Check className="w-3.5 h-3.5" />
                            {t('common.success')}
                        </button>
                    </>
                )}
            </div>
        </div>
    );

    return (
        <div className="flex-1 overflow-auto bg-surface-base h-full outline-none" ref={scrollerRef}>
            <div className="sticky top-0 z-10 border-b border-stroke-divider bg-surface-base px-6 pb-4 pt-6">
                <div className="w-full">
                    <div className="mb-4 flex items-center gap-3">
                        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-accent-light2">
                            <Calendar className="h-4.5 w-4.5 text-accent-default" />
                        </div>
                        <h1 className="font-ui text-fs-xl font-bold text-text-primary">{t('nav.schedule')}</h1>

                        <div className="ml-auto flex items-center gap-2">
                            {[
                                { label: t('schedule.active'), count: activeReminders.length, icon: <Calendar className="h-3.5 w-3.5" />, color: 'text-blue-500 bg-blue-500/10' },
                                { label: t('schedule.pending'), count: pendingReminders.length, icon: <Bell className="h-3.5 w-3.5" />, color: 'text-orange-500 bg-orange-500/10' },
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
                            {(['all', 'meeting', 'deliverable', 'event', 'appointment'] as const).map((f) => {
                                const active = typeFilter === f;
                                return (
                                    <button
                                        key={f}
                                        type="button"
                                        onClick={() => setTypeFilter(f)}
                                        className={`shrink-0 rounded-full px-3 py-1 text-fs-xs font-semibold uppercase transition-colors ${active
                                            ? 'bg-accent-default text-white'
                                            : 'bg-surface-subtle text-text-secondary hover:text-text-primary'
                                            }`}
                                    >
                                        {f === 'all' ? t('repository.filter_all') : f}
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                </div>
            </div>

            <div className="w-full max-w-6xl mx-auto p-6 lg:p-10 flex flex-col min-h-0">
                <div className="grid grid-cols-[36px_minmax(0,1fr)] items-start gap-6 lg:grid-cols-[minmax(124px,148px)_minmax(0,1fr)] flex-1">
                    <aside className="sticky top-[160px]">
                        <div className="relative pl-6 pb-2 pt-2">
                            <div className="absolute bottom-1 left-2 top-1 w-px bg-stroke-divider" />
                            <ul className="space-y-3">
                                {dayGroups.map((group) => {
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

                    <div className="min-w-0 space-y-8 pb-32 pt-2">
                        {dayGroups.map(group => (
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

                                {group.pending.length > 0 && (
                                    <div className="mb-5">
                                        <div className="mb-3 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                            <Bell className="h-3 w-3 text-orange-500" />
                                            {t('schedule.pending')}
                                            <span className="bg-orange-100 text-orange-600 text-[10px] px-1.5 py-0.5 rounded-full ml-1">
                                                {group.pending.length}
                                            </span>
                                        </div>
                                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                                            {group.pending.map(r => renderReminderCard(r, true))}
                                        </div>
                                    </div>
                                )}

                                <div className="mb-4">
                                    {(group.active.length > 0 || group.dateKey === todayKey) && (
                                        <div className="mb-3 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                            <Calendar className="h-3 w-3 text-accent-default" />
                                            {t('schedule.active')}
                                            {group.active.length > 0 && (
                                                <span className="bg-surface-subtle text-text-tertiary text-[10px] px-1.5 py-0.5 rounded-full ml-1">
                                                    {group.active.length}
                                                </span>
                                            )}
                                        </div>
                                    )}
                                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                                        {group.dateKey === todayKey && (
                                            <button
                                                type="button"
                                                onClick={() => {
                                                    toast.info('Feature coming soon');
                                                }}
                                                className="group/card relative flex h-full min-h-[120px] w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-accent-default/45 bg-accent-default/5 text-accent-default transition-all duration-200 hover:border-accent-default hover:bg-accent-default/10 hover:shadow-[var(--shadow-card-hover)]"
                                            >
                                                <span className="inline-flex h-9 w-9 items-center justify-center rounded-xl border border-accent-default/25 bg-accent-default/12 transition-transform duration-200 group-hover/card:scale-105">
                                                    <Plus className="h-4 w-4" />
                                                </span>
                                                <span className="text-sm font-semibold tracking-wide">{t('schedule.add')}</span>
                                            </button>
                                        )}
                                        {group.active.map(r => renderReminderCard(r, false))}
                                    </div>
                                </div>
                            </section>
                        ))}
                    </div>
                </div>
            </div>
        </div>
    );
};
