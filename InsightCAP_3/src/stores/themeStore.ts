/**
 * themeStore — 四主題切換
 * 主題 class 直接掛在 <html>，讓 CSS variable 生效
 * localStorage 持久化（Phase 1 先用 localStorage，Phase 2 後改用 settings 表）
 */

import { create } from 'zustand';

export type Theme = 'light' | 'dark' | 'casual' | 'fresh';

const STORAGE_KEY = 'ic-theme';
const THEME_CLASSES: Theme[] = ['light', 'dark', 'casual', 'fresh'];

function applyTheme(theme: Theme) {
    const html = document.documentElement;
    THEME_CLASSES.forEach(t => html.classList.remove(`theme-${t}`));
    html.classList.add(`theme-${theme}`);
}

function readStoredTheme(): Theme {
    try {
        const stored = localStorage.getItem(STORAGE_KEY) as Theme | null;
        if (stored && THEME_CLASSES.includes(stored)) return stored;
    } catch {
        // localStorage 不可用
    }
    return 'light'; // 預設淺色系（架構文件規定）
}

// 同步套用初始主題（index.html 的內聯 script 已先行套用，這裡確保 store 同步）
const initialTheme = readStoredTheme();
applyTheme(initialTheme);

interface ThemeState {
    theme: Theme;
    setTheme: (theme: Theme) => void;
}

export const useThemeStore = create<ThemeState>(() => ({
    theme: initialTheme,

    setTheme: (theme: Theme) => {
        applyTheme(theme);
        try {
            localStorage.setItem(STORAGE_KEY, theme);
        } catch {
            // ignore
        }
        useThemeStore.setState({ theme });
    },
}));
