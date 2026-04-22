import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Zap, AlertCircle, Send } from 'lucide-react';
import { useThemeStore } from '../stores/themeStore';
import { useLanguageStore } from '../stores/languageStore';
import { useT } from '../hooks/useT';

export function QuickCapturePage() {
    const t = useT();
    const { theme } = useThemeStore();
    const { language } = useLanguageStore(); // Subscribe to language changes to trigger re-render
    const [text, setText] = useState('');
    const [status, setStatus] = useState<'idle' | 'error'>('idle');
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        const unlisten = listen('show-quick-capture', () => {
            setText('');
            setStatus('idle');
            setTimeout(() => inputRef.current?.focus(), 50);
        });

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                getCurrentWindow().hide();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        setTimeout(() => inputRef.current?.focus(), 100);

        return () => {
            unlisten.then(fn => fn());
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, []);

    async function handleSubmit() {
        const trimmed = text.trim();
        if (!trimmed) {
            getCurrentWindow().hide();
            return;
        }

        try {
            await invoke('quick_capture', { content: trimmed });
            getCurrentWindow().hide();
        } catch (e) {
            console.error('[QuickCapture] Error:', e);
            setStatus('error');
            setTimeout(() => setStatus('idle'), 2000);
        }
    }

    async function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            await handleSubmit();
        }
    }

    const StatusIcon = () => {
        if (status === 'error') return <AlertCircle className="w-4 h-4 text-red-500 animate-pulse" />;
        return <Zap className="w-4 h-4 text-accent-default" />;
    };

    const placeholder = status === 'error'
        ? t('capture.save_failed')
        : t('capture.quick_input_placeholder');

    return (
        <div className="w-screen h-screen flex items-center px-3 bg-transparent overflow-hidden">
            <div
                className="w-full h-[52px] flex items-center gap-3 bg-surface-flyout border border-stroke-card rounded-xl px-4 transition-all duration-300"
            >
                <div className="shrink-0 flex items-center justify-center">
                    <StatusIcon />
                </div>

                <input
                    ref={inputRef}
                    value={text}
                    onChange={e => setText(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder={placeholder}
                    disabled={status === 'error'}
                    className={`flex-1 bg-transparent border-none outline-none text-fs-sm font-ui leading-none placeholder:text-text-tertiary transition-colors ${status === 'error' ? 'text-red-500' : 'text-text-primary'
                        }`}
                />

                <button
                    onClick={handleSubmit}
                    disabled={!text || status !== 'idle'}
                    className={`shrink-0 w-8 h-8 flex items-center justify-center rounded-lg transition-all duration-200 ${text && status === 'idle'
                        ? 'text-accent-default bg-accent-default/10 hover:bg-accent-default hover:text-white cursor-pointer active:scale-95'
                        : 'text-text-tertiary opacity-40 cursor-not-allowed'
                        }`}
                >
                    <Send className="w-3.5 h-3.5" />
                </button>
            </div>
        </div>
    );
}
