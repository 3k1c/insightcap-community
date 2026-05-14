import { useEffect, useState } from 'react';
import { Toaster } from 'sonner';
import { invoke } from '@tauri-apps/api/core';
import { SetupPage } from './pages/SetupPage';
import { LoginPage } from './pages/LoginPage';
import { MigratePage } from './pages/MigratePage';
import { MainLayout } from './components/layout/MainLayout';
import { QuickCapturePage } from './pages/QuickCapturePage';
import { DecisionReviewToast } from './components/memory/DecisionToast';
import { ReminderToast } from './components/memory/ReminderToast';

const windowLabel = (window as unknown as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } }).__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? '';

interface StartupStatus {
    kbPath: string;
    isSetup: boolean;
    autoLogin: boolean;
    isMigrated: boolean;
}

type AppState = 'loading' | 'setup' | 'login' | 'migrate' | 'main';

export default function App() {
    if (windowLabel === 'quick-capture') {
        return <QuickCapturePage />;
    }

    const [appState, setAppState] = useState<AppState>('loading');
    const [kbPath, setKbPath] = useState('');

    useEffect(() => {
        initApp();
    }, []);

    async function initApp() {
        try {
            const status = await invoke<StartupStatus>('get_startup_status');
            setKbPath(status.kbPath);

            if (!status.isSetup) {
                setAppState('setup');
            } else if (status.isMigrated) {
                setAppState('migrate');
            } else if (status.autoLogin) {
                setAppState('main');
            } else {
                setAppState('login');
            }
        } catch (e) {
            console.error('[App] Failed to init:', e);
            setAppState('setup');
        }
    }

    if (appState === 'loading') {
        return (
            <div className="flex h-screen w-screen items-center justify-center bg-surface-base text-text-primary">
                <div className="flex flex-col items-center gap-4 text-center">
                    <div className="h-8 w-8 animate-spin rounded-full border-2 border-stroke-divider border-t-accent-default" />
                    <div>
                        <p className="text-fs-lg font-semibold">正在啟動 InsightCAP</p>
                        <p className="mt-2 text-fs-sm text-text-secondary">首次啟動可能需要幾秒鐘，請稍候。</p>
                    </div>
                </div>
            </div>
        );
    }

    if (appState === 'setup') {
        return (
            <>
                <SetupPage onComplete={initApp} />
                <Toaster
                    position="top-right" offset={80}
                    toastOptions={{
                        style: {
                            backgroundColor: 'var(--surface-base)',
                            color: 'var(--text-primary)',
                            borderColor: 'var(--stroke-divider)',
                            boxShadow: 'var(--shadow-flyout)'
                        },
                        className: '!bg-surface-base !text-text-primary !border-stroke-divider'
                    }}
                />
            </>
        );
    }

    if (appState === 'migrate') {
        return (
            <>
                <MigratePage kbPath={kbPath} onUnlockSuccess={initApp} />
                <Toaster
                    position="top-right" offset={80}
                    toastOptions={{
                        style: {
                            backgroundColor: 'var(--surface-base)',
                            color: 'var(--text-primary)',
                            borderColor: 'var(--stroke-divider)',
                            boxShadow: 'var(--shadow-flyout)'
                        },
                        className: '!bg-surface-base !text-text-primary !border-stroke-divider'
                    }}
                />
            </>
        );
    }

    if (appState === 'login') {
        return (
            <>
                <LoginPage
                    kbPath={kbPath}
                    onLoginSuccess={() => setAppState('main')}
                />
                <Toaster
                    position="top-right" offset={80}
                    toastOptions={{
                        style: {
                            backgroundColor: 'var(--surface-base)',
                            color: 'var(--text-primary)',
                            borderColor: 'var(--stroke-divider)',
                            boxShadow: 'var(--shadow-flyout)'
                        },
                        className: '!bg-surface-base !text-text-primary !border-stroke-divider'
                    }}
                />
            </>
        );
    }

    if (appState === 'main') {
        return (
            <>
                <MainLayout />
                <DecisionReviewToast />
                <ReminderToast />
                <Toaster
                    position="top-right" offset={80}
                    toastOptions={{
                        style: {
                            backgroundColor: 'var(--surface-base)',
                            color: 'var(--text-primary)',
                            borderColor: 'var(--stroke-divider)',
                            boxShadow: 'var(--shadow-flyout)'
                        },
                        className: '!bg-surface-base !text-text-primary !border-stroke-divider'
                    }}
                />
            </>
        );
    }

    return null;
}
