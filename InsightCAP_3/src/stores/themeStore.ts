/**
 * themeStore — 四主題切換
 * 主題 class 直接掛在 <html>，讓 CSS variable 生效
 * localStorage 持久化（Phase 1 先用 localStorage，Phase 2 後改用 settings 表）
 *
 * 主題對應：
 *   frost → Frost Glass（亮色・冰藍，預設）
 *   void  → Deep Void（暗色・靛紫）
 *   warm  → Warm Parchment（亮色・琥珀）
 *   sage  → Sage Breeze（亮色・草地綠）
 */

import { create } from 'zustand';

export type Theme = 'frost' | 'void' | 'warm' | 'sage';

const STORAGE_KEY = 'ic-theme';
const THEME_CLASSES: Theme[] = ['frost', 'void', 'warm', 'sage'];

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
    return 'frost'; // 預設 Frost Glass（架構文件規定）
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
