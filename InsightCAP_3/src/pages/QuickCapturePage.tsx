import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function QuickCapturePage() {
    const [text, setText] = useState('');
    const [status, setStatus] = useState<'idle' | 'saving' | 'done' | 'error'>('idle');
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        // 每次視窗顯示後，自動聚焦輸入框並清空
        const unlisten = listen('show-quick-capture', () => {
            setText('');
            setStatus('idle');
            setTimeout(() => inputRef.current?.focus(), 50);
        });

        // 按下 Escape 隱藏視窗
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                getCurrentWindow().hide();
            }
        };
        window.addEventListener('keydown', handleKeyDown);

        // 初次掛載也聚焦
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

        setStatus('saving');
        try {
            // 直接呼叫 quick_capture command 寫入 inbox
            await invoke('quick_capture', { content: trimmed });
            setStatus('done');
            setTimeout(() => {
                setText('');
                setStatus('idle');
                getCurrentWindow().hide();
            }, 800);
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

    const statusIcon = status === 'saving' ? '⏳' : status === 'done' ? '✅' : status === 'error' ? '❌' : '⚡';
    const placeholder = status === 'done' ? '已儲存！' : status === 'error' ? '儲存失敗，請重試' : '輸入想法、連結、或任何內容... (Enter 送出, Esc 取消)';

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
                    background: 'rgba(20, 20, 30, 0.92)',
                    backdropFilter: 'blur(20px)',
                    borderRadius: '14px',
                    border: '1px solid rgba(255,255,255,0.12)',
                    boxShadow: '0 8px 32px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05)',
                    padding: '10px 16px',
                }}
            >
                <span style={{ fontSize: '18px', flexShrink: 0 }}>{statusIcon}</span>
                <input
                    ref={inputRef}
                    value={text}
                    onChange={e => setText(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder={placeholder}
                    disabled={status === 'saving' || status === 'done'}
                    style={{
                        flex: 1,
                        background: 'transparent',
                        border: 'none',
                        outline: 'none',
                        color: status === 'done' ? '#4ade80' : status === 'error' ? '#f87171' : 'rgba(255,255,255,0.9)',
                        fontSize: '15px',
                        fontFamily: 'Inter, "Microsoft JhengHei", sans-serif',
                        letterSpacing: '0.01em',
                    }}
                />
                {text && status === 'idle' && (
                    <button
                        onClick={handleSubmit}
                        style={{
                            flexShrink: 0,
                            background: 'rgba(99, 102, 241, 0.8)',
                            color: 'white',
                            border: 'none',
                            borderRadius: '8px',
                            padding: '4px 12px',
                            fontSize: '13px',
                            cursor: 'pointer',
                            whiteSpace: 'nowrap',
                        }}
                    >
                        儲存 ↵
                    </button>
                )}
            </div>
        </div>
    );
}
