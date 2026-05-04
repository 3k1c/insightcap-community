import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { LANGUAGE_STORAGE_KEY } from '../i18n';
import { SetupPage } from './SetupPage';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

describe('SetupPage', () => {
    it('allows choosing the app language during first setup', async () => {
        localStorage.removeItem(LANGUAGE_STORAGE_KEY);

        render(<SetupPage onComplete={vi.fn()} />);

        const languageSelect = screen.getByRole('combobox', { name: '介面語言' });

        await userEvent.selectOptions(languageSelect, 'en');

        expect(localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe('en');
    });
});
