import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

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

    it('uses the same dark theme fallback in the startup HTML', () => {
        const html = readFileSync(resolve(process.cwd(), 'index.html'), 'utf8');

        expect(html).toContain("? stored : 'void'");
        expect(html).toContain("classList.add('theme-void')");
    });
});
