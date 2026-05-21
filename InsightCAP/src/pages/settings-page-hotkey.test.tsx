import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { SettingsPage } from './SettingsPage';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({
    getVersion: vi.fn(async () => '0.9.0-beta.1'),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(async () => vi.fn()),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
    save: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openUrl: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);
const mockGetVersion = vi.mocked(getVersion);

const settings = {
    general: {
        launchAtStartup: false,
        minimizeToTray: true,
        language: 'zh-TW',
    },
    aiModels: {
        chatLlm: { provider: 'ollama', model: 'gemma4:e4b', baseUrl: 'http://localhost:11434' },
        contentProcessorLlm: { provider: 'ollama', model: 'gemma4:e4b', baseUrl: 'http://localhost:11434' },
        visionModel: { provider: 'ollama', model: 'gemma4:e4b', baseUrl: 'http://localhost:11434' },
        embeddingModel: { provider: 'local', model: 'MultilingualE5Small' },
        speechToTextModel: { provider: 'local', model: 'base' },
        summaryModel: 'follow_chat',
        providerProfiles: [],
    },
    knowledge: {
        kbPath: 'C:\\Data\\InsightCAP',
        autoClassifyEnabled: true,
        autoSpaceMode: 'suggest',
    },
    hotkeys: {
        captureClipboard: 'Ctrl+Alt+F',
        quickInput: 'Ctrl+Alt+G',
    },
    autoCleanup: {
        enabled: false,
        retentionDays: 0,
    },
    webSearch: {
        enabled: false,
        provider: 'tavily',
        apiKey: '',
    },
    editor: {
        defaultFont: 'Inter',
        defaultFontSize: '16',
        defaultLineSpacing: '1.6',
        defaultExportFormat: 'docx',
        exportSubdir: 'exports',
        aiActions: [],
    },
    telegram: {
        botToken: '',
        allowedUserIds: [],
        enabled: false,
    },
    reminders: {
        aiEnabled: true,
        enabled: true,
        dailyReminderTime: '09:00',
        quietHoursStart: '22:00',
        quietHoursEnd: '08:00',
        weekendQuiet: false,
    },
    aiUsage: {
        mode: 'balanced',
    },
    chatPromptInstruction: '',
};

describe('SettingsPage hotkey recorder', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'get_settings') return settings;
            if (command === 'whisper_model_status') return {};
            return undefined;
        });
        mockGetVersion.mockResolvedValue('0.9.0-beta.1');
    });

    it('pauses global shortcuts while recording and resumes them after a new quick input hotkey is captured', async () => {
        const user = userEvent.setup();

        render(<SettingsPage />);

        const quickInputButton = await screen.findByRole('button', { name: 'Ctrl+Alt+G' });
        await user.click(quickInputButton);

        await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('pause_global_hotkeys'));

        fireEvent.keyDown(quickInputButton, {
            key: 'h',
            code: 'KeyH',
            ctrlKey: true,
            altKey: true,
        });

        expect(await screen.findByRole('button', { name: 'CommandOrControl+Alt+H' })).toBeInTheDocument();
        await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('resume_global_hotkeys'));
    });

    it('renders the actual app version in the settings sidebar footer', async () => {
        render(<SettingsPage />);

        expect(await screen.findByText('InsightCAP v0.9.0-beta.1')).toBeInTheDocument();
        expect(screen.queryByText('InsightCAP v0.1.0')).not.toBeInTheDocument();
    });
});
