import React, { useEffect, useState, useMemo, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useT } from '../hooks/useT';
import { useThemeStore } from '../stores/themeStore';
import { Calendar, Clock, Check, X, Bell, Plus, Search, Tag, FileText, Layers, ChevronDown } from 'lucide-react';
import { toast } from 'sonner';

// --- Sub-components ---

const CustomSelect: React.FC<{
    value: string;
    options: { value: string; label: string }[];
    onChange: (value: any) => void;
    icon: React.ReactNode;
}> = ({ value, options, onChange, icon }) => {
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const selectedOption = options.find(o => o.value === value) || options[0];

    return (
        <div className="relative" ref={containerRef}>
            <button
                type="button"
                onClick={() => setIsOpen(!isOpen)}
                className="flex h-12 w-full items-center justify-between rounded-2xl border border-stroke-control bg-surface-control px-4 text-fs-base text-text-primary outline-none transition-all hover:bg-surface-control-hover focus:border-accent-default focus:ring-4 focus:ring-accent-default/10"
            >
                <div className="flex items-center gap-3">
                    <span className="text-text-tertiary">{icon}</span>
                    <span>{selectedOption.label}</span>
                </div>
                <ChevronDown className={`h-4 w-4 text-text-tertiary transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`} />
            </button>

            {isOpen && (
                <div className="absolute left-0 top-full z-[100] mt-2 w-full animate-in fade-in zoom-in-95 duration-200">
                    <div className="overflow-hidden rounded-2xl border border-stroke-divider bg-surface-flyout p-1.5 shadow-xl ring-1 ring-black/5">
                        {options.map((opt) => (
                            <button
                                key={opt.value}
                                type="button"
                                onClick={() => {
                                    onChange(opt.value);
                                    setIsOpen(false);
                                }}
                                className={`flex w-full items-center px-3 py-2.5 text-fs-sm rounded-xl transition-colors ${opt.value === value
                                    ? 'bg-accent-default text-white'
                                    : 'text-text-primary hover:bg-surface-subtle'
                                    }`}
                            >
                                {opt.label}
                            </button>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
};

// --- Main Page ---

interface Reminder {
    id: string;
    title: string;
    description: string | null;
    eventType: 'meeting' | 'deliverable' | 'event' | 'appointment' | string;
    eventDate: string | null;
    eventTime: string | null;
    status: string;
    pendingConfirm: number;
}

const typeStyles: Record<string, { color: string, bg: string, icon: React.ReactNode }> = {
    meeting: {
        color: 'text-blue-500',
        bg: 'bg-blue-500/10 dark:bg-blue-500/20',
        icon: <Bell className="h-4 w-4" />
    },
    deliverable: {
        color: 'text-emerald-500',
        bg: 'bg-emerald-500/10 dark:bg-emerald-500/20',
        icon: <Check className="h-4 w-4" />
    },
    event: {
        color: 'text-purple-500',
        bg: 'bg-purple-500/10 dark:bg-purple-500/20',
        icon: <Layers className="h-4 w-4" />
    },
    appointment: {
        color: 'text-amber-500',
        bg: 'bg-amber-500/10 dark:bg-amber-500/20',
        icon: <Clock className="h-4 w-4" />
    },
    default: {
        color: 'text-text-secondary',
        bg: 'bg-surface-subtle',
        icon: <Bell className="h-4 w-4" />
    }
};

interface ReminderDraft {
    title: string;
    description: string;
    eventType: 'meeting' | 'deliverable' | 'event' | 'appointment';
    eventDate: string;
    eventTime: string;
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
    const { theme } = useThemeStore();
    const isDark = theme === 'void';

    const [activeReminders, setActiveReminders] = useState<Reminder[]>([]);
    const [pendingReminders, setPendingReminders] = useState<Reminder[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [selectedDateKey, setSelectedDateKey] = useState<string | null>(null);
    const [query, setQuery] = useState('');
    const [typeFilter, setTypeFilter] = useState<'all' | 'meeting' | 'deliverable' | 'event' | 'appointment'>('all');
    const [isCreateOpen, setIsCreateOpen] = useState(false);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [draft, setDraft] = useState<ReminderDraft>({
        title: '',
        description: '',
        eventType: 'event',
        eventDate: new Date().toISOString().slice(0, 10),
        eventTime: '',
    });

    const scrollerRef = useRef<HTMLDivElement | null>(null);
    const sectionRefs = useRef<Record<string, HTMLElement | null>>({});
    const markerRefs = useRef<Record<string, HTMLSpanElement | null>>({});
    const firstRowRefs = useRef<Record<string, HTMLDivElement | null>>({});

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

    const openCreateReminder = (dateKey?: string) => {
        setDraft({
            title: '',
            description: '',
            eventType: 'event',
            eventDate: dateKey && dateKey !== 'unknown' ? dateKey : todayKey,
            eventTime: '',
        });
        setIsCreateOpen(true);
    };

    const closeCreateReminder = () => {
        if (isSubmitting) return;
        setIsCreateOpen(false);
    };

    const handleCreateReminder = async () => {
        if (!draft.title.trim()) {
            toast.error(t('schedule.toast_title_required'));
            return;
        }

        if (!draft.eventDate) {
            toast.error(t('schedule.toast_date_required'));
            return;
        }

        setIsSubmitting(true);
        try {
            await invoke('create_reminder', {
                input: {
                    title: draft.title.trim(),
                    description: draft.description.trim() || null,
                    event_type: draft.eventType,
                    event_date: draft.eventDate,
                    event_time: draft.eventTime || null,
                },
            });
            toast.success(t('schedule.toast_create_success'));
            setIsCreateOpen(false);
            await loadReminders();
            setSelectedDateKey(draft.eventDate);
        } catch (error) {
            console.error('Create reminder failed:', error);
            toast.error(typeof error === 'string' ? error : t('schedule.toast_create_failed'));
        } finally {
            setIsSubmitting(false);
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
        const firstRow = firstRowRefs.current[dateKey];
        const section = sectionRefs.current[dateKey];

        if (!scroller || !section) return;

        if (!marker) {
            const target = firstRow || section;
            if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
            return;
        }

        const targetEl = firstRow || section;
        if (!targetEl) return;

        const markerRect = marker.getBoundingClientRect();
        const targetRect = targetEl.getBoundingClientRect();
        const markerCenterY = markerRect.top + markerRect.height / 2;
        const targetCenterY = targetRect.top + targetRect.height / 2;
        const nextTop = scroller.scrollTop + (targetCenterY - markerCenterY) - 50;

        scroller.scrollTo({ top: Math.max(0, nextTop), behavior: 'smooth' });
    };

    const renderReminderCard = (r: Reminder, pending: boolean) => {
        const style = typeStyles[r.eventType] || typeStyles.default;

        return (
            <div key={r.id} className="group/card relative flex min-h-[180px] flex-col overflow-hidden rounded-2xl border border-stroke-card bg-surface-layer p-4 shadow-sm transition-all duration-300 hover:-translate-y-1 hover:shadow-[var(--shadow-card-hover)] cursor-default">
                {/* Top Section: Icon & Title */}
                <div className="flex items-start gap-3">
                    <div className={`shrink-0 flex h-10 w-10 items-center justify-center rounded-xl ${style.bg} ${style.color} shadow-sm transition-transform duration-300 group-hover/card:scale-105`}>
                        {style.icon}
                    </div>
                    <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2 mb-0.5">
                            <span className="text-[10px] font-bold uppercase tracking-widest text-text-tertiary">
                                {t(`reminder.type_${r.eventType}`) || r.eventType}
                            </span>
                            {pending && (
                                <span className="inline-flex h-2 w-2 rounded-full bg-orange-500 animate-pulse" />
                            )}
                        </div>
                        <h3 className="line-clamp-2 text-[14px] font-semibold leading-tight text-text-primary">
                            {r.title}
                        </h3>
                    </div>
                </div>

                {/* Middle: Description */}
                <div className="mt-3 flex-1">
                    {r.description ? (
                        <p className="line-clamp-3 text-[12.5px] leading-relaxed text-text-tertiary break-words">
                            {r.description}
                        </p>
                    ) : (
                        <div className="h-4 border-l-2 border-stroke-divider ml-5 opacity-40" />
                    )}
                </div>

                {/* Metadata Section: Date & Time */}
                <div className="mt-4 flex flex-wrap items-center gap-2">
                    {r.eventDate && (
                        <div className="inline-flex items-center gap-1.5 rounded-full bg-surface-subtle/60 px-2.5 py-1 text-[11px] font-medium text-text-secondary border border-stroke-card/50">
                            <Calendar className="h-3 w-3 opacity-70" />
                            {r.eventDate}
                        </div>
                    )}
                    {r.eventTime && (
                        <div className="inline-flex items-center gap-1.5 rounded-full bg-surface-subtle/60 px-2.5 py-1 text-[11px] font-medium text-text-secondary border border-stroke-card/50">
                            <Clock className="h-3 w-3 opacity-70" />
                            {r.eventTime}
                        </div>
                    )}
                </div>

                {/* Action Buttons: Overlay on hover or constant for pending */}
                <div className={`mt-4 flex items-center justify-end gap-2 pt-3 border-t border-stroke-divider transition-opacity duration-200 ${pending ? 'opacity-100' : 'opacity-0 group-hover/card:opacity-100'}`}>
                    {pending ? (
                        <>
                            <button
                                onClick={() => handleConfirm(r.id, false)}
                                className="inline-flex h-8 items-center gap-1.5 rounded-lg px-3 text-[12px] font-semibold text-text-tertiary transition-all hover:bg-red-500/10 hover:text-red-500"
                            >
                                <X className="h-3.5 w-3.5" />
                                {t('common.cancel')}
                            </button>
                            <button
                                onClick={() => handleConfirm(r.id, true)}
                                className="inline-flex h-8 items-center gap-1.5 rounded-lg bg-accent-default px-4 text-[12px] font-bold text-white shadow-sm transition-all hover:bg-accent-hover active:scale-95"
                            >
                                <Check className="h-3.5 w-3.5" />
                                {t('common.confirm')}
                            </button>
                        </>
                    ) : (
                        <>
                            <button
                                onClick={() => handleUpdateStatus(r.id, 'dismissed')}
                                className="inline-flex h-8 items-center gap-1.5 rounded-lg px-3 text-[12px] font-semibold text-text-tertiary transition-all hover:bg-red-500/10 hover:text-red-500"
                            >
                                <X className="h-3.5 w-3.5" />
                                {t('common.cancel')}
                            </button>
                            <button
                                onClick={() => handleUpdateStatus(r.id, 'completed')}
                                className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 text-[12px] font-bold text-emerald-600 transition-all hover:bg-emerald-500/20 hover:text-emerald-700 active:scale-95"
                            >
                                <Check className="h-3.5 w-3.5" />
                                {t('common.completed') || '已完成'}
                            </button>
                        </>
                    )}
                </div>
            </div>
        );
    };

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

            <div className="w-full px-6 py-5">
                <div className="grid grid-cols-[36px_minmax(0,1fr)] items-start gap-6 lg:grid-cols-[minmax(124px,148px)_minmax(0,1fr)]">
                    <aside className="sticky top-[160px]">
                        <div className="relative pl-6 pb-2 pt-32">
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

                    <div className="min-w-0 space-y-4 pb-64 pt-2">
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
                                    <div className="mb-3">
                                        <div className="mb-2 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                            <Bell className="h-3 w-3 text-orange-500" />
                                            {t('schedule.pending')}
                                            <span className="bg-orange-100 text-orange-600 text-[10px] px-1.5 py-0.5 rounded-full ml-1">
                                                {group.pending.length}
                                            </span>
                                        </div>
                                        <div
                                            ref={(el) => { if (!firstRowRefs.current[group.dateKey]) firstRowRefs.current[group.dateKey] = el; }}
                                            className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3"
                                        >
                                            {group.pending.map(r => renderReminderCard(r, true))}
                                        </div>
                                    </div>
                                )}

                                <div className="mb-3">
                                    {(group.active.length > 0 || group.dateKey === todayKey) && (
                                        <div className="mb-2 flex items-center gap-1.5 text-fs-xs font-semibold uppercase tracking-wider text-text-tertiary">
                                            <Calendar className="h-3 w-3 text-accent-default" />
                                            {t('schedule.active')}
                                            {group.active.length > 0 && (
                                                <span className="bg-surface-subtle text-text-tertiary text-[10px] px-1.5 py-0.5 rounded-full ml-1">
                                                    {group.active.length}
                                                </span>
                                            )}
                                        </div>
                                    )}
                                    <div
                                        ref={(el) => { if (group.pending.length === 0) firstRowRefs.current[group.dateKey] = el; }}
                                        className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3"
                                    >
                                        {group.dateKey === todayKey && (
                                            <button
                                                type="button"
                                                onClick={() => openCreateReminder(group.dateKey)}
                                                className="group/card relative flex h-full min-h-[160px] w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-accent-default/45 bg-accent-default/5 text-accent-default transition-all duration-200 hover:border-accent-default hover:bg-accent-default/10 hover:shadow-[var(--shadow-card-hover)]"
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

            {isCreateOpen && (
                <div
                    className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-[2px] transition-all duration-300 animate-in fade-in"
                    onClick={closeCreateReminder}
                >
                    <div
                        className="w-full max-w-xl overflow-hidden rounded-3xl border border-stroke-divider bg-surface-layer shadow-[0_20px_50px_-12px_rgba(0,0,0,0.3)] animate-in zoom-in-95 duration-200"
                        onClick={(e) => e.stopPropagation()}
                    >
                        {/* Header with gradient background */}
                        <div className="relative bg-gradient-to-br from-accent-default/10 via-transparent to-transparent px-8 pt-8 pb-6">
                            <div className="flex items-start justify-between">
                                <div className="space-y-1">
                                    <h2 className="text-2xl font-bold tracking-tight text-text-primary">
                                        {t('schedule.add_title')}
                                    </h2>
                                    <p className="text-fs-sm text-text-tertiary">
                                        {t('schedule.add_subtitle')}
                                    </p>
                                </div>
                                <button
                                    type="button"
                                    onClick={closeCreateReminder}
                                    className="flex h-9 w-9 items-center justify-center rounded-full bg-surface-subtle text-text-tertiary transition-all hover:bg-surface-control hover:text-text-primary"
                                >
                                    <X className="h-5 w-5" />
                                </button>
                            </div>
                        </div>

                        <div className="px-8 pb-8 space-y-6">
                            <div className="space-y-4">
                                <div className="space-y-2">
                                    <label className="flex items-center gap-2 text-fs-sm font-semibold text-text-secondary">
                                        <FileText className="h-4 w-4" />
                                        {t('schedule.title_label')}
                                    </label>
                                    <input
                                        type="text"
                                        value={draft.title}
                                        onChange={(e) => setDraft((current) => ({ ...current, title: e.target.value }))}
                                        placeholder={t('schedule.title_placeholder')}
                                        className="h-12 w-full rounded-2xl border border-stroke-control bg-surface-control px-4 text-fs-base text-text-primary outline-none transition-all placeholder:text-text-tertiary focus:border-accent-default focus:ring-4 focus:ring-accent-default/10"
                                    />
                                </div>

                                <div className="space-y-2">
                                    <label className="flex items-center gap-2 text-fs-sm font-semibold text-text-secondary">
                                        <Layers className="h-4 w-4" />
                                        {t('schedule.desc_label')}
                                    </label>
                                    <textarea
                                        value={draft.description}
                                        onChange={(e) => setDraft((current) => ({ ...current, description: e.target.value }))}
                                        rows={3}
                                        placeholder={t('schedule.desc_placeholder')}
                                        className="w-full rounded-2xl border border-stroke-control bg-surface-control px-4 py-3 text-fs-base text-text-primary outline-none transition-all placeholder:text-text-tertiary focus:border-accent-default focus:ring-4 focus:ring-accent-default/10 resize-none"
                                    />
                                </div>

                                <div className="grid grid-cols-1 gap-5 sm:grid-cols-2">
                                    <div className="space-y-2">
                                        <label className="flex items-center gap-2 text-fs-sm font-semibold text-text-secondary">
                                            <Tag className="h-4 w-4" />
                                            {t('schedule.type_label')}
                                        </label>
                                        <CustomSelect
                                            value={draft.eventType}
                                            options={[
                                                { value: 'event', label: t('reminder.type_event') },
                                                { value: 'meeting', label: t('reminder.type_meeting') },
                                                { value: 'deliverable', label: t('reminder.type_deliverable') },
                                                { value: 'appointment', label: t('reminder.type_appointment') },
                                            ]}
                                            onChange={(val) => setDraft((c) => ({ ...c, eventType: val }))}
                                            icon={<Tag className="h-4 w-4" />}
                                        />
                                    </div>

                                    <div className="space-y-2">
                                        <label className="flex items-center gap-2 text-fs-sm font-semibold text-text-secondary">
                                            <Calendar className="h-4 w-4" />
                                            {t('schedule.date_label')}
                                        </label>
                                        <input
                                            type="date"
                                            value={draft.eventDate}
                                            onChange={(e) => setDraft((current) => ({ ...current, eventDate: e.target.value }))}
                                            style={{ colorScheme: isDark ? 'dark' : 'light' }}
                                            className="h-12 w-full rounded-2xl border border-stroke-control bg-surface-control px-4 text-fs-base text-text-primary outline-none transition-all focus:border-accent-default focus:ring-4 focus:ring-accent-default/10"
                                        />
                                    </div>
                                </div>

                                <div className="space-y-2">
                                    <label className="flex items-center gap-2 text-fs-sm font-semibold text-text-secondary">
                                        <Clock className="h-4 w-4" />
                                        {t('schedule.time_label')}
                                    </label>
                                    <input
                                        type="time"
                                        value={draft.eventTime}
                                        onChange={(e) => setDraft((current) => ({ ...current, eventTime: e.target.value }))}
                                        style={{ colorScheme: isDark ? 'dark' : 'light' }}
                                        className="h-12 w-full rounded-2xl border border-stroke-control bg-surface-control px-4 text-fs-base text-text-primary outline-none transition-all focus:border-accent-default focus:ring-4 focus:ring-accent-default/10"
                                    />
                                    <div className="flex items-center gap-1.5 px-1 py-1 text-fs-xs text-text-tertiary">
                                        <Bell className="h-3.5 w-3.5 shrink-0 text-amber-500" />
                                        <span>{t('schedule.time_hint')}</span>
                                    </div>
                                </div>
                            </div>

                            <div className="flex items-center justify-end gap-3 pt-4">
                                <button
                                    type="button"
                                    onClick={closeCreateReminder}
                                    className="h-11 rounded-2xl px-6 text-fs-base font-semibold text-text-secondary transition-all hover:bg-surface-subtle hover:text-text-primary"
                                >
                                    {t('common.cancel')}
                                </button>
                                <button
                                    type="button"
                                    onClick={handleCreateReminder}
                                    disabled={isSubmitting}
                                    className="group relative flex h-11 items-center gap-2 overflow-hidden rounded-2xl bg-accent-default px-8 text-fs-base font-bold text-white shadow-lg transition-all hover:bg-accent-hover hover:shadow-accent-default/20 disabled:scale-[0.98] disabled:opacity-60 disabled:shadow-none"
                                >
                                    {isSubmitting ? (
                                        <>
                                            <div className="h-4 w-4 animate-spin rounded-full border-2 border-white/20 border-t-white" />
                                            <span>{t('schedule.creating')}</span>
                                        </>
                                    ) : (
                                        <>
                                            <Plus className="h-4.5 w-4.5 transition-transform group-hover:rotate-90" />
                                            <span>{t('schedule.create_button')}</span>
                                        </>
                                    )}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
