import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import i18n, { LANGUAGE_STORAGE_KEY } from '../i18n';
import { SetupPage } from './SetupPage';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(open);

function setupInvoke() {
    mockInvoke.mockImplementation(async (command: string) => {
        if (command === 'setup_auth') return 'alpha beta gamma delta';
        if (command === 'get_auth_status') {
            return {
                isSetup: false,
                autoLogin: false,
                isMigrated: false,
            };
        }
        return undefined;
    });
}

async function goToWorkspace(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole('button', { name: /Start Setup/i }));
    expect(screen.getByRole('heading', { name: /Choose Knowledge Base Location/i })).toBeInTheDocument();
}

async function chooseWorkspace(user: ReturnType<typeof userEvent.setup>) {
    mockOpen.mockResolvedValue('C:\\Data\\InsightCAP');
    await user.click(screen.getByRole('button', { name: /Choose folder/i }));
    await user.click(screen.getByRole('button', { name: /^Next/i }));
}

async function fillValidPassword(user: ReturnType<typeof userEvent.setup>) {
    await user.type(screen.getByLabelText(/^Master Password/i), 'password123');
    await user.type(screen.getByLabelText(/^Confirm Password/i), 'password123');
}

async function enterAiStep(user: ReturnType<typeof userEvent.setup>) {
    await goToWorkspace(user);
    await chooseWorkspace(user);
    await fillValidPassword(user);
    await user.click(screen.getByRole('button', { name: /^Next/i }));
    expect(screen.getByRole('heading', { name: /Connect AI Service/i })).toBeInTheDocument();
}

async function enterRecoveryBySkippingAi(user: ReturnType<typeof userEvent.setup>) {
    await enterAiStep(user);
    await user.click(screen.getByRole('button', { name: /Skip and Create/i }));
    await screen.findByText('alpha beta gamma delta');
}

describe('SetupPage onboarding flow', () => {
    beforeEach(async () => {
        localStorage.removeItem(LANGUAGE_STORAGE_KEY);
        await i18n.changeLanguage('en');
        vi.clearAllMocks();
        setupInvoke();
    });

    it('allows choosing the app language during first setup', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await user.selectOptions(screen.getByRole('combobox', { name: /Interface Language/i }), 'en');

        expect(localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe('en');
    });

    it('renders the app logo instead of the IC monogram in the setup header', () => {
        render(<SetupPage onComplete={vi.fn()} />);

        expect(screen.getByAltText('InsightCAP logo')).toBeInTheDocument();
        expect(screen.queryByText(/^IC$/)).not.toBeInTheDocument();
    });

    it('renders onboarding copy in Traditional Chinese without English fallback text', async () => {
        const user = userEvent.setup();
        await i18n.changeLanguage('zh-TW');

        render(<SetupPage onComplete={vi.fn()} />);

        expect(screen.getByRole('button', { name: /開始設定/i })).toBeInTheDocument();
        expect(screen.queryByText('Start Setup')).not.toBeInTheDocument();
        expect(screen.queryByText('Welcome')).not.toBeInTheDocument();
        expect(screen.queryByText('Workspace')).not.toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: /開始設定/i }));

        expect(screen.getByRole('heading', { name: /選擇知識庫位置/i })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /上一步/i })).toBeInTheDocument();
        expect(screen.queryByRole('button', { name: /^Back$/i })).not.toBeInTheDocument();
    });

    it('uses the safe step order: welcome, workspace, password, AI, recovery', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        expect(screen.getByText('Step 1 / 5')).toBeInTheDocument();
        expect(screen.getAllByRole('heading', { name: /Welcome to InsightCAP/i }).length).toBeGreaterThan(0);

        await goToWorkspace(user);
        expect(screen.getByText('Step 2 / 5')).toBeInTheDocument();

        await chooseWorkspace(user);
        expect(screen.getByText('Step 3 / 5')).toBeInTheDocument();
        expect(screen.getByRole('heading', { name: /Set Master Password/i })).toBeInTheDocument();

        await fillValidPassword(user);
        await user.click(screen.getByRole('button', { name: /^Next/i }));
        expect(screen.getByText('Step 4 / 5')).toBeInTheDocument();
        expect(screen.getByRole('heading', { name: /Connect AI Service/i })).toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: /Skip and Create/i }));
        expect(await screen.findByText('Step 5 / 5')).toBeInTheDocument();
        expect(screen.getByRole('heading', { name: /Save Your Recovery Phrase/i })).toBeInTheDocument();
    });

    it('blocks setup when the selected workspace already contains a knowledge base', async () => {
        const user = userEvent.setup();
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'get_auth_status') {
                return {
                    isSetup: true,
                    autoLogin: false,
                    isMigrated: false,
                };
            }
            if (command === 'setup_auth') return 'alpha beta gamma delta';
            return undefined;
        });

        render(<SetupPage onComplete={vi.fn()} />);

        await goToWorkspace(user);
        await chooseWorkspace(user);

        expect(screen.getByRole('heading', { name: /Choose Knowledge Base Location/i })).toBeInTheDocument();
        expect(screen.getByText(/already contains an InsightCAP knowledge base/i)).toBeInTheDocument();
        expect(mockInvoke.mock.calls.some(([command]) => command === 'setup_auth')).toBe(false);
    });

    it('calls setup_auth once after AI step when skipping AI', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterRecoveryBySkippingAi(user);

        const setupCalls = mockInvoke.mock.calls.filter(([command]) => command === 'setup_auth');
        expect(setupCalls).toHaveLength(1);
        expect(setupCalls[0][1]).toMatchObject({
            payload: {
                password: 'password123',
                kbPath: 'C:\\Data\\InsightCAP',
                autoLogin: true,
            },
        });
        expect(JSON.stringify(setupCalls[0][1])).not.toContain('initialSettings');
        expect(screen.getByText('alpha beta gamma delta')).toBeInTheDocument();
    });

    it('includes initialSettings.aiModels when AI settings are provided', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterAiStep(user);
        await user.selectOptions(screen.getByLabelText(/Provider Type/i), 'openai');
        await user.clear(screen.getByLabelText(/Model Name/i));
        await user.type(screen.getByLabelText(/Model Name/i), 'gpt-4o-mini');
        await user.type(screen.getByLabelText(/^API Key$/i, { selector: 'input' }), 'sk-test');
        await user.click(screen.getByRole('button', { name: /Create and Continue/i }));

        await screen.findByText('alpha beta gamma delta');

        const setupCalls = mockInvoke.mock.calls.filter(([command]) => command === 'setup_auth');
        expect(setupCalls).toHaveLength(1);
        const payload = setupCalls[0][1] as { payload: any };
        expect(payload.payload.initialSettings.aiModels.chatLlm).toMatchObject({
            provider: 'openai',
            model: 'gpt-4o-mini',
            apiKey: 'sk-test',
            baseUrl: '',
        });
        expect(payload.payload.initialSettings.aiModels.providerProfiles[0]).toMatchObject({
            provider: 'openai',
            apiKey: 'sk-test',
            baseUrl: '',
        });
        expect(JSON.stringify(payload)).not.toContain('base_url');
    });

    it('treats the primary AI action as skip when an API-key provider has no API key', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterAiStep(user);
        await user.click(screen.getByRole('button', { name: /Create and Continue/i }));
        await screen.findByText('alpha beta gamma delta');

        const setupCalls = mockInvoke.mock.calls.filter(([command]) => command === 'setup_auth');
        expect(setupCalls).toHaveLength(1);
        expect(JSON.stringify(setupCalls[0][1])).not.toContain('initialSettings');
    });

    it('includes Ollama AI settings without requiring an API key', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterAiStep(user);
        await user.selectOptions(screen.getByLabelText(/Provider Type/i), 'ollama');
        await user.click(screen.getByRole('button', { name: /Create and Continue/i }));
        await screen.findByText('alpha beta gamma delta');

        const setupCalls = mockInvoke.mock.calls.filter(([command]) => command === 'setup_auth');
        const payload = setupCalls[0][1] as { payload: any };
        expect(payload.payload.initialSettings.aiModels.chatLlm).toMatchObject({
            provider: 'ollama',
            model: 'qwen2.5:7b',
            baseUrl: 'http://localhost:11434',
        });
        expect(payload.payload.initialSettings.aiModels.providerProfiles[0]).toMatchObject({
            provider: 'ollama',
            apiKey: '',
            baseUrl: 'http://localhost:11434',
        });
    });

    it('does not call get_settings or save_settings during onboarding', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterRecoveryBySkippingAi(user);

        expect(mockInvoke.mock.calls.some(([command]) => command === 'get_settings')).toBe(false);
        expect(mockInvoke.mock.calls.some(([command]) => command === 'save_settings')).toBe(false);
    });

    it('does not allow changing earlier setup after setup_auth succeeds', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterRecoveryBySkippingAi(user);

        expect(screen.queryByRole('button', { name: /Back/i })).not.toBeInTheDocument();
        expect(screen.queryByRole('heading', { name: /Connect AI Service/i })).not.toBeInTheDocument();
        expect(screen.queryByRole('heading', { name: /Set Master Password/i })).not.toBeInTheDocument();
        expect(screen.queryByRole('heading', { name: /Choose Knowledge Base Location/i })).not.toBeInTheDocument();
    });

    it('requires recovery confirmation before finishing and then restarts', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterRecoveryBySkippingAi(user);
        const finishButton = screen.getByRole('button', { name: /Finish Setup/i });
        expect(finishButton).toBeDisabled();

        await user.click(screen.getByLabelText(/I have safely saved/i));
        await user.click(finishButton);

        expect(mockInvoke).toHaveBeenCalledWith('confirm_new_recovery', { kbPath: 'C:\\Data\\InsightCAP' });
        expect(mockInvoke).toHaveBeenCalledWith('restart_app');
    });

    it('validates password length and confirmation before AI step', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await goToWorkspace(user);
        await chooseWorkspace(user);

        await user.type(screen.getByLabelText(/^Master Password/i), 'short');
        expect(screen.getByRole('button', { name: /^Next/i })).toBeDisabled();
        expect(screen.getByText(/at least 8 characters/i)).toBeInTheDocument();

        await user.clear(screen.getByLabelText(/^Master Password/i));
        await user.type(screen.getByLabelText(/^Master Password/i), 'password123');
        await user.type(screen.getByLabelText(/^Confirm Password/i), 'different');
        expect(screen.getByRole('button', { name: /^Next/i })).toBeDisabled();
        expect(screen.getByText(/Passwords do not match/i)).toBeInTheDocument();

        expect(mockInvoke.mock.calls.some(([command]) => command === 'setup_auth')).toBe(false);
    });

    it('toggles API key visibility', async () => {
        const user = userEvent.setup();

        render(<SetupPage onComplete={vi.fn()} />);

        await enterAiStep(user);
        const apiKeyInput = screen.getByLabelText(/^API Key$/i, { selector: 'input' });
        expect(apiKeyInput).toHaveAttribute('type', 'password');

        await user.click(screen.getByRole('button', { name: /Show API key/i }));
        expect(apiKeyInput).toHaveAttribute('type', 'text');

        await user.click(screen.getByRole('button', { name: /Hide API key/i }));
        expect(apiKeyInput).toHaveAttribute('type', 'password');
    });
});
