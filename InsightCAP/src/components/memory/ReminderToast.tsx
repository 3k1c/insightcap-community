import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { X, Check, Clock, BellOff } from 'lucide-react';
import { useT } from '../../hooks/useT';

interface ReminderNotification {
    notificationId: string;
    reminderId: string;
    intent: string;
    title: string;
    eventType: string;
    dateStatus: string;
    eventDate: string | null;
    eventTime: string | null;
}

const INTENT_KEYS: Record<string, string> = {
    start: 'reminder.intent_start',
    midcheck: 'reminder.intent_midcheck',
    urgent: 'reminder.intent_urgent',
    final: 'reminder.intent_final',
    prepare: 'reminder.intent_prepare',
    imminent: 'reminder.intent_imminent',
    now: 'reminder.intent_now',
    confirm_date: 'reminder.intent_confirm_date',
};

export function ReminderToast() {
    const t = useT();
    const [notification, setNotification] = useState<ReminderNotification | null>(null);
    const [processing, setProcessing] = useState(false);

    useEffect(() => {
        let unlisten: (() => void) | null = null;
        listen<ReminderNotification>('reminder-notification', (event) => {
            setNotification(event.payload);
        }).then(fn => { unlisten = fn; });
        return () => { unlisten?.(); };
    }, []);

    if (!notification) return null;

    const intentKey = INTENT_KEYS[notification.intent] || 'reminder.intent_start';
    const isUrgent = notification.intent === 'urgent' || notification.intent === 'final' || notification.intent === 'now';

    async function handleComplete() {
        if (!notification) return;
        setProcessing(true);
        try {
            await invoke('update_reminder_status', { reminderId: notification.reminderId, status: 'completed' });
        } catch (e) {
            console.error('Failed to complete reminder:', e);
        }
        setNotification(null);
        setProcessing(false);
    }

    async function handleSnooze(minutes: number) {
        if (!notification) return;
        setProcessing(true);
        try {
            await invoke('snooze_reminder', { reminderId: notification.reminderId, snoozeMinutes: minutes });
        } catch (e) {
            console.error('Failed to snooze reminder:', e);
        }
        setNotification(null);
        setProcessing(false);
    }

    async function handleDismiss() {
        if (!notification) return;
        setProcessing(true);
        try {
            await invoke('update_reminder_status', { reminderId: notification.reminderId, status: 'dismissed' });
        } catch (e) {
            console.error('Failed to dismiss reminder:', e);
        }
        setNotification(null);
        setProcessing(false);
    }

    return (
        <div className="fixed bottom-20 right-4 z-[100] w-[360px] animate-in slide-in-from-right-5">
            <div className={`rounded-xl border shadow-lg p-4 ${isUrgent ? 'bg-red-500/5 border-red-500/20' : 'bg-surface-base border-stroke-divider'}`}>
                {/* Header */}
                <div className="flex items-start justify-between mb-2">
                    <div className="flex items-center gap-2">
                        <Clock className={`w-4 h-4 ${isUrgent ? 'text-red-500' : 'text-accent-default'}`} />
                        <span className="text-fs-xs text-text-tertiary">{t(intentKey as never)}</span>
                    </div>
                    <button onClick={handleDismiss} disabled={processing} className="text-text-tertiary hover:text-text-primary">
                        <X className="w-4 h-4" />
                    </button>
                </div>

                {/* Title */}
                <h4 className="text-fs-base font-semibold text-text-primary mb-1">{notification.title}</h4>

                {/* Date info */}
                {notification.eventDate && (
                    <p className="text-fs-sm text-text-secondary mb-3">
                        {notification.eventDate}
                        {notification.eventTime ? ` ${notification.eventTime}` : ''}
                    </p>
                )}

                {/* Actions */}
                <div className="flex items-center gap-2">
                    <button
                        onClick={handleComplete}
                        disabled={processing}
                        className="flex items-center gap-1 px-3 py-1.5 rounded-lg text-fs-xs bg-green-500/10 text-green-600 border border-green-500/20 hover:bg-green-500/20 transition-colors disabled:opacity-50"
                    >
                        <Check className="w-3 h-3" />
                        {t('reminder.action_complete')}
                    </button>
                    <button
                        onClick={() => handleSnooze(30)}
                        disabled={processing}
                        className="flex items-center gap-1 px-3 py-1.5 rounded-lg text-fs-xs bg-blue-500/10 text-blue-600 border border-blue-500/20 hover:bg-blue-500/20 transition-colors disabled:opacity-50"
                    >
                        <Clock className="w-3 h-3" />
                        {t('reminder.snooze_30min')}
                    </button>
                    <button
                        onClick={() => handleSnooze(60)}
                        disabled={processing}
                        className="flex items-center gap-1 px-3 py-1.5 rounded-lg text-fs-xs bg-blue-500/10 text-blue-600 border border-blue-500/20 hover:bg-blue-500/20 transition-colors disabled:opacity-50"
                    >
                        <Clock className="w-3 h-3" />
                        {t('reminder.snooze_1hr')}
                    </button>
                    <button
                        onClick={handleDismiss}
                        disabled={processing}
                        className="flex items-center gap-1 px-3 py-1.5 rounded-lg text-fs-xs text-text-tertiary hover:text-text-primary transition-colors disabled:opacity-50"
                    >
                        <BellOff className="w-3 h-3" />
                    </button>
                </div>
            </div>
        </div>
    );
}
