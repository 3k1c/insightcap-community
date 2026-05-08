
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
    }
    return 'void'; // Default dark mode.
}

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
        }
        useThemeStore.setState({ theme });
    },
}));

window.addEventListener('storage', (e) => {
    if (e.key === STORAGE_KEY && e.newValue) {
        const theme = e.newValue as Theme;
        if (THEME_CLASSES.includes(theme)) {
            applyTheme(theme);
            useThemeStore.setState({ theme });
        }
    }
});
