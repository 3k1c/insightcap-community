import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('themeStore', () => {
    beforeEach(() => {
        vi.resetModules();
        localStorage.clear();
        document.documentElement.className = '';
    });

    it('defaults to dark mode when no stored theme exists', async () => {
        const { useThemeStore } = await import('./themeStore');

        expect(useThemeStore.getState().theme).toBe('void');
        expect(document.documentElement.classList.contains('theme-void')).toBe(true);
    });
});
