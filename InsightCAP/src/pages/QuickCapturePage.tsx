import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Zap, AlertCircle, Send } from 'lucide-react';

export function QuickCapturePage() {
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
        if (status === 'error') return <AlertCircle className="w-4 h-4" style={{ color: '#DC2626' }} />;
        return <Zap className="w-4 h-4" style={{ color: 'var(--accent-default)' }} />;
    };

    const placeholder = status === 'error'
        ? '儲存失敗，請重試'
        : '輸入想法、連結、或任何內容… (Enter 送出, Esc 取消)';

    const inputColor = status === 'error' ? '#DC2626' : 'var(--text-primary)';

    return (
        <div
            style={{
                width: '100vw',
                height: '100vh',
                display: 'flex',
                alignItems: 'center',
                padding: '0 12px',
                background: 'transparent',
            }}
        >
            <div
                style={{
                    width: '100%',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    background: 'var(--surface-flyout)',
                    borderRadius: '12px',
                    border: '1px solid var(--stroke-card)',
                    padding: '10px 16px',
                }}
            >
                <StatusIcon />

                <input
                    ref={inputRef}
                    value={text}
                    onChange={e => setText(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder={placeholder}
                    disabled={status === 'error'}
                    style={{
                        flex: 1,
                        background: 'transparent',
                        border: 'none',
                        outline: 'none',
                        color: inputColor,
                        fontSize: '14px',
                        fontFamily: 'var(--font-ui)',
                        lineHeight: '20px',
                        letterSpacing: '0.01em',
                    }}
                />

                <button
                    onClick={handleSubmit}
                    disabled={!text || status !== 'idle'}
                    style={{
                        flexShrink: 0,
                        background: 'transparent',
                        border: 'none',
                        borderRadius: '8px',
                        padding: '8px',
                        lineHeight: 0,
                        cursor: text && status === 'idle' ? 'pointer' : 'not-allowed',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: text && status === 'idle' ? 'var(--text-tertiary)' : 'var(--text-tertiary)',
                        opacity: text && status === 'idle' ? 1 : 0.4,
                        transition: 'opacity 150ms ease-out',
                    }}
                >
                    <Send className="w-4 h-4" />
                </button>
            </div>
        </div>
    );
}
