import { useEffect, useState } from 'react';
import { Toaster } from 'sonner';
import { invoke } from '@tauri-apps/api/core';
import { SetupPage } from './pages/SetupPage';
import { LoginPage } from './pages/LoginPage';
import { MainLayout } from './components/layout/MainLayout';
import { QuickCapturePage } from './pages/QuickCapturePage';

// 偵測當前視窗 label
const windowLabel = (window as unknown as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } }).__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? '';

// Quick Capture 視窗直接渲染，跳過認證
if (windowLabel === 'quick-capture') {
    import('./design-system/index.css');
}


interface AuthStatus {
    isSetup: boolean;
    autoLogin: boolean;
}

type AppState = 'loading' | 'setup' | 'login' | 'main';

export default function App() {
    // Quick Capture 視窗直接渲染，跳過認證流程
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
            // 取得工作區路徑（從 settings 或 bootstrap.json）
            const path = await getKbPath();
            setKbPath(path);

            const status = await invoke<AuthStatus>('get_auth_status', { kbPath: path });

            if (!status.isSetup) {
                setAppState('setup');
            } else if (status.autoLogin) {
                // 自動登入模式：直接嘗試登入
                const autoOk = await invoke<boolean>('try_auto_login', { kbPath: path });
                if (autoOk) {
                    setAppState('main');
                } else {
                    setAppState('login');
                }
            } else {
                setAppState('login');
            }
        } catch (e) {
            console.error('[App] Failed to init:', e);
            setAppState('setup');
        }
    }

    async function getKbPath(): Promise<string> {
        try {
            // Phase 1: 從 settings 讀取 kb_path
            // 暫時使用 app data dir 作為預設值
            const settings = await invoke<{ kbPath?: string }>('get_settings').catch(() => ({}));
            if (settings && (settings as { knowledge?: { kbPath?: string } }).knowledge) {
                return (settings as { knowledge?: { kbPath?: string } }).knowledge?.kbPath || '';
            }
        } catch { }
        return '';
    }

    if (appState === 'loading') {
        return (
            <div
                className="flex h-screen w-screen items-center justify-center"
                style={{ background: 'var(--ic-bg-base)' }}
            >
                <div
                    className="h-8 w-8 animate-spin rounded-full border-2 border-t-transparent"
                    style={{ borderColor: 'var(--ic-border-default)', borderTopColor: 'var(--ic-accent)' }}
                />
            </div>
        );
    }

    if (appState === 'setup') {
        return (
            <>
                <SetupPage onComplete={initApp} />
                <Toaster position="bottom-right" />
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
                <Toaster position="bottom-right" />
            </>
        );
    }

    if (appState === 'main') {
        return (
            <>
                <MainLayout />
                <Toaster position="bottom-right" />
            </>
        );
    }

    return null;
}
