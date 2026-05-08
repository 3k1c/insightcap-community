import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    AlertTriangle,
    Check,
    ChevronLeft,
    ChevronRight,
    Copy,
    Database,
    Eye,
    EyeOff,
    FolderOpen,
    KeyRound,
    Search,
    ShieldCheck,
    Sparkles,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { useLanguageStore } from '../stores/languageStore';
import type { Language } from '../i18n';
import appLogo from '../../src-tauri/icons/128x128.png';

type Step = 'welcome' | 'workspace' | 'password' | 'ai_provider' | 'recovery';
type SetupMode = 'new' | 'existing';

interface SetupPageProps {
    onComplete: () => void;
}

interface ProviderProfileData {
    id: string;
    name: string;
    provider: string;
    baseUrl?: string;
    apiKey?: string;
}

interface InitialSettings {
    aiModels: {
        chatLlm: {
            provider: string;
            model: string;
            apiKey?: string;
            baseUrl?: string;
        };
        contentProcessorLlm: {
            provider: string;
            model: string;
            baseUrl?: string;
        };
        visionModel: {
            provider: string;
            model: string;
            baseUrl?: string;
        };
        embeddingModel: {
            provider: string;
            model: string;
        };
        speechToTextModel: {
            provider: string;
            model: string;
        };
        providerProfiles: ProviderProfileData[];
    };
}

interface AuthStatus {
    isSetup: boolean;
    autoLogin: boolean;
    isMigrated: boolean;
    isEmptyForNewSetup?: boolean;
}

const stepOrder: Step[] = ['welcome', 'workspace', 'password', 'ai_provider', 'recovery'];

const providerLabels: Record<string, string> = {
    openai: 'OpenAI',
    anthropic: 'Anthropic',
    google: 'Google',
    xai: 'xAI (Grok)',
    openrouter: 'OpenRouter',
    ollama: 'Ollama (Local)',
};

function providerBaseUrl(provider: string) {
    return provider === 'ollama' ? 'http://localhost:11434' : '';
}

function providerNeedsApiKey(provider: string) {
    return provider !== 'ollama';
}

function buildInitialSettings(provider: string, model: string, apiKey: string): InitialSettings {
    const cleanKey = apiKey.trim();
    const baseUrl = providerBaseUrl(provider);
    return {
        aiModels: {
            chatLlm: {
                provider,
                model,
                baseUrl,
                ...(providerNeedsApiKey(provider) ? { apiKey: cleanKey } : {}),
            },
            contentProcessorLlm: {
                provider: 'ollama',
                model: 'qwen2.5:3b',
                baseUrl: 'http://localhost:11434',
            },
            visionModel: {
                provider: 'ollama',
                model: 'deepseek-ocr',
                baseUrl: 'http://localhost:11434',
            },
            embeddingModel: {
                provider: 'local',
                model: 'MultilingualE5Small',
            },
            speechToTextModel: {
                provider: 'local',
                model: 'base',
            },
            providerProfiles: [{
                id: crypto.randomUUID(),
                name: providerLabels[provider] ?? provider,
                provider,
                baseUrl,
                apiKey: providerNeedsApiKey(provider) ? cleanKey : '',
            }],
        },
    };
}

export function SetupPage({ onComplete }: SetupPageProps) {
    const { t } = useTranslation();
    const { language, setLanguage } = useLanguageStore();

    const [step, setStep] = useState<Step>('welcome');
    const [setupMode, setSetupMode] = useState<SetupMode>('new');
    const [workspacePath, setWorkspacePath] = useState('');
    const [password, setPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [restoreMnemonic, setRestoreMnemonic] = useState('');
    const [showPassword, setShowPassword] = useState(false);
    const [showConfirmPassword, setShowConfirmPassword] = useState(false);
    const [recoveryPhrase, setRecoveryPhrase] = useState('');
    const [recoveryConfirmed, setRecoveryConfirmed] = useState(false);
    const [copied, setCopied] = useState(false);
    const [setupProvider, setSetupProvider] = useState('openai');
    const [setupModel, setSetupModel] = useState('gpt-4o-mini');
    const [setupApiKey, setSetupApiKey] = useState('');
    const [showApiKey, setShowApiKey] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    const stepIndex = stepOrder.indexOf(step);
    const isPasswordLongEnough = password.length >= 8;
    const passwordsMatch = !confirmPassword || password === confirmPassword;
    const canContinuePassword = isPasswordLongEnough && password === confirmPassword;
    const shouldIncludeAiSettings = setupProvider === 'ollama' || setupApiKey.trim().length > 0;

    const steps = useMemo(() => [
        { id: 'welcome' as const, label: tr('auth.setup.step_label_welcome', 'Welcome') },
        { id: 'workspace' as const, label: tr('auth.setup.step_label_workspace', 'Workspace') },
        { id: 'password' as const, label: tr('auth.setup.step_label_security', 'Security') },
        { id: 'ai_provider' as const, label: tr('auth.setup.step_label_ai', 'AI Service') },
        { id: 'recovery' as const, label: tr('auth.setup.step_label_recovery', 'Recovery') },
    ], [t]);

    function tr(key: string, fallback: string) {
        return t(key, { defaultValue: fallback });
    }

    function formatSetupError(errorValue: unknown) {
        const message = String(errorValue);
        if (message.includes('WORKSPACE_ALREADY_INITIALIZED')) {
            return tr(
                'auth.setup.workspace_existing',
                'This folder already contains an InsightCAP knowledge base. Choose a new folder for first setup.'
            );
        }
        if (message.includes('WORKSPACE_NOT_EMPTY')) {
            return tr(
                'auth.setup.workspace_not_empty',
                'New knowledge bases can only be created in an empty folder. Choose another empty folder, or manually move or delete this folder contents first.'
            );
        }
        if (message.includes('INVALID_MNEMONIC')) {
            return tr('auth.setup.restore_invalid_mnemonic', 'Incorrect recovery phrase, please try again.');
        }
        return message;
    }

    async function handleBrowse() {
        try {
            const selected = await openDialog({ directory: true });
            if (selected) setWorkspacePath(selected as string);
        } catch {
        }
    }

    async function handleWorkspaceNext() {
        if (loading) return;
        setError('');
        const kbPath = workspacePath.trim();
        if (!kbPath) {
            setError(tr('auth.setup.workspace_required', 'Please enter workspace path'));
            return;
        }

        setLoading(true);
        try {
            const authStatus = await invoke<AuthStatus>('get_auth_status', { kbPath });
            const hasExistingKb = authStatus.isSetup || authStatus.isMigrated;
            if (setupMode === 'existing') {
                if (!hasExistingKb) {
                    setError(tr(
                        'auth.setup.workspace_missing_existing',
                        'This folder does not contain an existing InsightCAP knowledge base.'
                    ));
                    return;
                }
                setStep('password');
                return;
            }
            if (hasExistingKb) {
                setError(tr(
                    'auth.setup.workspace_existing',
                    'This folder already contains an InsightCAP knowledge base. Choose a new folder for first setup.'
                ));
                return;
            }
            if (authStatus.isEmptyForNewSetup === false) {
                setError(tr(
                    'auth.setup.workspace_not_empty',
                    'New knowledge bases can only be created in an empty folder. Choose another empty folder, or manually move or delete this folder contents first.'
                ));
                return;
            }
            setStep('password');
        } catch (e) {
            setError(formatSetupError(e));
        } finally {
            setLoading(false);
        }
    }

    function handlePasswordNext() {
        setError('');
        if (!canContinuePassword) {
            setError(!isPasswordLongEnough
                ? tr('auth.setup.password_too_short', 'Password must be at least 8 characters')
                : tr('auth.setup.password_mismatch', 'Passwords do not match'));
            return;
        }
        setStep('ai_provider');
    }

    async function handleRestoreExistingKb() {
        setError('');
        const cleanMnemonic = restoreMnemonic.trim();
        if (!cleanMnemonic) {
            setError(tr('auth.setup.restore_mnemonic_required', 'Please enter your recovery phrase.'));
            return;
        }
        if (!canContinuePassword) {
            setError(!isPasswordLongEnough
                ? tr('auth.setup.password_too_short', 'Password must be at least 8 characters')
                : tr('auth.setup.password_mismatch', 'Passwords do not match'));
            return;
        }

        setLoading(true);
        try {
            const mnemonic = await invoke<string>('unlock_migrated_with_mnemonic', {
                kbPath: workspacePath,
                mnemonic: cleanMnemonic,
                newPassword: password,
            });
            setRecoveryPhrase(mnemonic);
            setStep('recovery');
        } catch (e) {
            setError(formatSetupError(e));
        } finally {
            setLoading(false);
        }
    }

    async function handleCreateRecovery(includeAiSettings: boolean) {
        if (loading || recoveryPhrase) {
            setStep('recovery');
            return;
        }
        setError('');
        if (!canContinuePassword) {
            setError(!isPasswordLongEnough
                ? tr('auth.setup.password_too_short', 'Password must be at least 8 characters')
                : tr('auth.setup.password_mismatch', 'Passwords do not match'));
            return;
        }

        setLoading(true);
        try {
            const payload: {
                displayName: string;
                password: string;
                autoLogin: boolean;
                kbPath: string;
                initialSettings?: InitialSettings;
            } = {
                displayName: 'User',
                password,
                autoLogin: true,
                kbPath: workspacePath,
            };

            if (includeAiSettings && shouldIncludeAiSettings) {
                payload.initialSettings = buildInitialSettings(setupProvider, setupModel, setupApiKey);
            }

            const mnemonic = await invoke<string>('setup_auth', {
                payload,
            });
            setRecoveryPhrase(mnemonic);
            setStep('recovery');
        } catch (e) {
            setError(formatSetupError(e));
        } finally {
            setLoading(false);
        }
    }

    async function handleSkipAiAndCreate() {
        await handleCreateRecovery(false);
    }

    async function handleCreateRecoveryWithAi() {
        await handleCreateRecovery(shouldIncludeAiSettings);
    }

    async function handleCopyRecovery() {
        try {
            await navigator.clipboard.writeText(recoveryPhrase);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch {
        }
    }

    async function completeOnboarding() {
        try {
            await invoke('confirm_new_recovery', { kbPath: workspacePath });
            await invoke('restart_app');
        } catch {
            onComplete();
        }
    }

    const stepTitle = {
        welcome: tr('auth.setup.title', 'Welcome to InsightCAP'),
        workspace: tr('auth.setup.workspace_title', 'Choose Knowledge Base Location'),
        password: setupMode === 'existing'
            ? tr('auth.setup.restore_title', 'Restore Existing Knowledge Base')
            : tr('auth.setup.password_title', 'Set Master Password'),
        recovery: tr('auth.setup.recovery_title', 'Save Your Recovery Phrase'),
        ai_provider: tr('auth.setup.ai_title', 'Connect AI Service'),
    }[step];

    const stepDescription = {
        welcome: tr('auth.setup.welcome_subtitle', 'Create your local AI knowledge base assistant.'),
        workspace: tr('auth.setup.workspace_desc', 'InsightCAP stores captured content, indexes, and settings in this folder.'),
        password: setupMode === 'existing'
            ? tr('auth.setup.restore_desc', 'Enter the recovery phrase for this knowledge base and set a new master password for this device.')
            : tr('auth.setup.password_desc', 'This password protects your local data. InsightCAP does not store or upload your password.'),
        recovery: tr('auth.setup.recovery_hint', 'This recovery phrase is shown only once. If you forget your master password, you must use it to reset access.'),
        ai_provider: tr('auth.setup.ai_provider_desc', 'Set up a provider and model so InsightCAP can help organize, search, and generate content. You can also do this later in Settings.'),
    }[step];

    const setupDarkThemeVars = {
        '--surface-base': '#111827',
        '--surface-layer': '#161B26',
        '--surface-card': '#1F2937',
        '--surface-subtle': 'rgba(255, 255, 255, 0.06)',
        '--surface-control': 'rgba(255, 255, 255, 0.08)',
        '--surface-control-hover': 'rgba(255, 255, 255, 0.12)',
        '--text-primary': 'rgba(248, 250, 252, 0.94)',
        '--text-secondary': 'rgba(203, 213, 225, 0.78)',
        '--text-tertiary': 'rgba(148, 163, 184, 0.76)',
        '--stroke-divider': 'rgba(148, 163, 184, 0.16)',
        '--stroke-control': 'rgba(148, 163, 184, 0.22)',
        '--stroke-focus': '#818CF8',
    } as React.CSSProperties;

    return (
        <div
            className="min-h-screen w-screen bg-[radial-gradient(circle_at_top,#1E1B4B_0%,#070A12_42%,#030712_100%)] px-6 py-10 text-text-primary"
            style={setupDarkThemeVars}
        >
            <div className="mx-auto flex min-h-[calc(100vh-5rem)] max-w-3xl items-center justify-center">
                <section className="w-full max-w-[620px] rounded-2xl border border-white/10 bg-slate-900/92 p-9 shadow-[0_24px_70px_rgba(0,0,0,0.46)] backdrop-blur">
                    <header className="mb-7 text-center">
                        <div className="mx-auto mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-accent-default/10 text-fs-2xl font-semibold text-accent-default">
                            <img
                                src={appLogo}
                                alt="InsightCAP logo"
                                className="h-10 w-10 object-contain"
                            />
                        </div>
                        <h1 className="text-fs-2xl font-semibold text-text-primary">
                            {tr('auth.setup.title', 'Welcome to InsightCAP')}
                        </h1>
                        <p className="mt-1 text-fs-sm text-text-tertiary">
                            {tr('auth.setup.subtitle', 'Complete the initial setup to get started')}
                        </p>
                    </header>

                    <div className="mb-7">
                        <div className="mb-3 flex items-center justify-between">
                            <span className="text-fs-xs font-medium uppercase tracking-wide text-accent-default">
                                {tr('auth.setup.step_progress', 'Step {{current}} / 5').replace('{{current}}', String(stepIndex + 1))}
                            </span>
                            <span className="text-fs-xs text-text-tertiary">{steps[stepIndex].label}</span>
                        </div>
                        <div className="grid grid-cols-5 gap-2">
                            {steps.map((item, index) => {
                                const isCurrent = index === stepIndex;
                                const isDone = index < stepIndex;
                                return (
                                    <div key={item.id} className="min-w-0">
                                        <div className={`h-1.5 rounded-full transition-colors ${isDone || isCurrent ? 'bg-accent-default' : 'bg-stroke-divider'}`} />
                                        <div className={`mt-2 truncate text-center text-[11px] ${isCurrent ? 'font-semibold text-accent-default' : isDone ? 'text-accent-default' : 'text-text-tertiary'}`}>
                                            {item.label}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    </div>

                    <div className="mb-6">
                        <h2 className="text-fs-xl font-semibold text-text-primary">{stepTitle}</h2>
                        <p className="mt-2 text-fs-sm leading-6 text-text-secondary">{stepDescription}</p>
                    </div>

                    {step === 'welcome' && (
                        <div className="space-y-5">
                            <div className="rounded-xl border border-white/10 bg-slate-950/60 p-4">
                                <div className="grid gap-3 sm:grid-cols-2">
                                    {[
                                        [Database, tr('auth.setup.feature_capture', 'Capture clipboard content')],
                                        [Search, tr('auth.setup.feature_search', 'Organize and search knowledge')],
                                        [Sparkles, tr('auth.setup.feature_ai', 'Call AI quickly with hotkeys')],
                                        [ShieldCheck, tr('auth.setup.feature_local', 'Store data in your chosen location')],
                                    ].map(([Icon, label]) => {
                                        const FeatureIcon = Icon as typeof Database;
                                        return (
                                            <div key={String(label)} className="flex items-center gap-2 text-fs-sm text-slate-300">
                                                <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-indigo-500/15 text-indigo-300">
                                                    <FeatureIcon className="h-4 w-4" />
                                                </span>
                                                <span>{label as string}</span>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>

                            <div className="flex items-center justify-between rounded-xl border border-white/10 bg-slate-950/40 px-4 py-3">
                                <label htmlFor="setup-language" className="text-fs-sm font-medium text-slate-300">
                                    {tr('language.label', 'Interface Language')}
                                </label>
                                <select
                                    id="setup-language"
                                    value={language}
                                    onChange={e => setLanguage(e.target.value as Language)}
                                    className="h-9 rounded-lg border border-white/10 bg-slate-900 px-3 text-fs-sm text-slate-100 focus:outline-none focus:ring-1 focus:ring-indigo-400"
                                >
                                    <option value="zh-TW">{tr('language.zh-TW', 'Traditional Chinese')}</option>
                                    <option value="zh-CN">{tr('language.zh-CN', 'Simplified Chinese')}</option>
                                    <option value="en">{tr('language.en', 'English')}</option>
                                </select>
                            </div>

                            <div className="rounded-xl border border-white/10 bg-slate-950/60 p-4">
                                <div className="mb-3 text-fs-sm font-medium text-slate-100">
                                    {tr('auth.setup.welcome_hotkey_title', 'Global Hotkeys')}
                                </div>
                                <div className="space-y-2">
                                    <HotkeyRow label={tr('settings.capture_clipboard', 'Capture Clipboard')} value="Ctrl+Alt+F" />
                                    <HotkeyRow label={tr('settings.quick_input', 'Quick Input')} value="Ctrl+Alt+G" />
                                </div>
                                <p className="mt-3 text-fs-xs text-slate-400">
                                    {tr('auth.setup.welcome_hotkey_hint', 'You can customize hotkeys in Settings after setup is complete.')}
                                </p>
                            </div>

                            <Button className="h-10 w-full" onClick={() => setStep('workspace')}>
                                {tr('auth.setup.start_setup', 'Start Setup')} <ChevronRight className="h-4 w-4" />
                            </Button>
                        </div>
                    )}

                    {step === 'workspace' && (
                        <div className="space-y-5">
                            <div className="grid gap-3 sm:grid-cols-2">
                                <button
                                    type="button"
                                    onClick={() => { setSetupMode('new'); setError(''); }}
                                    className={`rounded-xl border p-4 text-left transition-colors ${setupMode === 'new' ? 'border-accent-default bg-accent-default/10' : 'border-stroke-divider bg-surface-base hover:bg-surface-subtle'}`}
                                >
                                    <div className="text-fs-sm font-semibold text-text-primary">
                                        {tr('auth.setup.mode_new_title', 'Create New Knowledge Base')}
                                    </div>
                                    <div className="mt-1 text-fs-xs leading-5 text-text-tertiary">
                                        {tr('auth.setup.mode_new_desc', 'Use an empty folder and create a new encrypted local knowledge base.')}
                                    </div>
                                </button>
                                <button
                                    type="button"
                                    onClick={() => { setSetupMode('existing'); setError(''); }}
                                    className={`rounded-xl border p-4 text-left transition-colors ${setupMode === 'existing' ? 'border-accent-default bg-accent-default/10' : 'border-stroke-divider bg-surface-base hover:bg-surface-subtle'}`}
                                >
                                    <div className="text-fs-sm font-semibold text-text-primary">
                                        {tr('auth.setup.mode_existing_title', 'Use Existing Knowledge Base')}
                                    </div>
                                    <div className="mt-1 text-fs-xs leading-5 text-text-tertiary">
                                        {tr('auth.setup.mode_existing_desc', 'Select an encrypted InsightCAP knowledge base and restore access with its recovery phrase.')}
                                    </div>
                                </button>
                            </div>
                            <div>
                                <label htmlFor="workspace-path" className="mb-1.5 block text-fs-sm font-medium text-text-primary">
                                    {tr('auth.setup.workspace_label', 'Workspace Path')}
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        id="workspace-path"
                                        type="text"
                                        value={workspacePath}
                                        onChange={e => setWorkspacePath(e.target.value)}
                                        placeholder={tr('auth.setup.workspace_placeholder', 'Choose or enter a folder path...')}
                                        className="h-10 flex-1 rounded-lg border border-stroke-divider bg-surface-base px-3 text-fs-sm text-text-primary placeholder:text-text-tertiary focus:outline-none focus:ring-1 focus:ring-stroke-focus"
                                    />
                                    <Button
                                        aria-label={tr('auth.setup.choose_folder', 'Choose folder')}
                                        variant="secondary"
                                        size="icon"
                                        className="h-10 w-10"
                                        onClick={handleBrowse}
                                    >
                                        <FolderOpen className="h-4 w-4" />
                                    </Button>
                                </div>
                                <p className="mt-2 text-fs-xs leading-5 text-text-tertiary">
                                    {tr('auth.setup.workspace_hint', 'Choose a local drive location that is easy to back up. Avoid temporary folders.')}
                                </p>
                            </div>
                            {error && <p className="text-fs-xs text-color-danger">{error}</p>}
                            <NavButtons
                                onBack={() => { setError(''); setStep('welcome'); }}
                                backLabel={tr('auth.setup.back', 'Back')}
                                nextLabel={tr('common.next', 'Next')}
                                onNext={handleWorkspaceNext}
                                nextDisabled={loading}
                                loading={loading}
                            />
                        </div>
                    )}

                    {step === 'password' && (
                        setupMode === 'existing' ? (
                            <div className="space-y-5">
                                <Input
                                    id="restore-recovery-phrase"
                                    label={tr('auth.setup.restore_mnemonic_label', 'Recovery Phrase')}
                                    value={restoreMnemonic}
                                    onChange={e => setRestoreMnemonic(e.target.value)}
                                    placeholder={tr('auth.setup.restore_mnemonic_placeholder', 'apple bridge cloud dance echo forest...')}
                                    hint={tr('auth.setup.restore_mnemonic_hint', 'Enter the 24-word recovery phrase saved when this knowledge base was created or last restored.')}
                                />
                                <Input
                                    id="restore-master-password"
                                    label={tr('auth.setup.restore_password_label', 'New Master Password')}
                                    type={showPassword ? 'text' : 'password'}
                                    value={password}
                                    onChange={e => setPassword(e.target.value)}
                                    placeholder={tr('auth.setup.password_placeholder', 'Enter password (at least 8 characters)')}
                                    error={password && !isPasswordLongEnough ? tr('auth.setup.password_too_short', 'Password must be at least 8 characters') : undefined}
                                    rightElement={
                                        <button
                                            type="button"
                                            aria-label={showPassword ? tr('auth.setup.hide_password', 'Hide password') : tr('auth.setup.show_password', 'Show password')}
                                            onClick={() => setShowPassword(!showPassword)}
                                            className="text-text-tertiary hover:text-text-primary"
                                        >
                                            {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                        </button>
                                    }
                                />
                                <Input
                                    id="restore-confirm-master-password"
                                    label={tr('auth.setup.restore_password_confirm_label', 'Confirm New Password')}
                                    type={showConfirmPassword ? 'text' : 'password'}
                                    value={confirmPassword}
                                    onChange={e => setConfirmPassword(e.target.value)}
                                    placeholder={tr('auth.setup.password_confirm_placeholder', 'Enter password again')}
                                    error={confirmPassword && !passwordsMatch ? tr('auth.setup.password_mismatch', 'Passwords do not match') : undefined}
                                    rightElement={
                                        <button
                                            type="button"
                                            aria-label={showConfirmPassword ? tr('auth.setup.hide_confirm_password', 'Hide confirmation password') : tr('auth.setup.show_confirm_password', 'Show confirmation password')}
                                            onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                                            className="text-text-tertiary hover:text-text-primary"
                                        >
                                            {showConfirmPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                        </button>
                                    }
                                />
                                {error && <p className="text-fs-xs text-color-danger">{error}</p>}
                                <NavButtons
                                    onBack={() => { setError(''); setStep('workspace'); }}
                                    backLabel={tr('auth.setup.back', 'Back')}
                                    nextLabel={tr('auth.setup.restore_kb', 'Restore Knowledge Base')}
                                    onNext={handleRestoreExistingKb}
                                    nextDisabled={loading || !restoreMnemonic.trim() || !canContinuePassword}
                                    loading={loading}
                                />
                            </div>
                        ) : (
                        <div className="space-y-5">
                            <Input
                                id="master-password"
                                label={tr('auth.setup.master_password_label', 'Master Password')}
                                type={showPassword ? 'text' : 'password'}
                                value={password}
                                onChange={e => setPassword(e.target.value)}
                                placeholder={tr('auth.setup.password_placeholder', 'Enter password (at least 8 characters)')}
                                error={password && !isPasswordLongEnough ? tr('auth.setup.password_too_short', 'Password must be at least 8 characters') : undefined}
                                hint={password && isPasswordLongEnough ? tr('auth.setup.password_strength_ok', 'Password strength: usable') : tr('auth.setup.password_strength_hint', 'Use at least 8 characters. Longer is better.')}
                                rightElement={
                                    <button
                                        type="button"
                                        aria-label={showPassword ? tr('auth.setup.hide_password', 'Hide password') : tr('auth.setup.show_password', 'Show password')}
                                        onClick={() => setShowPassword(!showPassword)}
                                        className="text-text-tertiary hover:text-text-primary"
                                    >
                                        {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                    </button>
                                }
                            />
                            <Input
                                id="confirm-master-password"
                                label={tr('auth.setup.password_confirm_label', 'Confirm Password')}
                                type={showConfirmPassword ? 'text' : 'password'}
                                value={confirmPassword}
                                onChange={e => setConfirmPassword(e.target.value)}
                                placeholder={tr('auth.setup.password_confirm_placeholder', 'Enter password again')}
                                error={confirmPassword && !passwordsMatch ? tr('auth.setup.password_mismatch', 'Passwords do not match') : undefined}
                                rightElement={
                                    <button
                                        type="button"
                                        aria-label={showConfirmPassword ? tr('auth.setup.hide_confirm_password', 'Hide confirmation password') : tr('auth.setup.show_confirm_password', 'Show confirmation password')}
                                        onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                                        className="text-text-tertiary hover:text-text-primary"
                                    >
                                        {showConfirmPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                    </button>
                                }
                            />
                            <div className="rounded-xl border border-accent-default/20 bg-accent-default/5 p-4 text-fs-sm leading-6 text-text-secondary">
                                <div className="mb-1 flex items-center gap-2 font-medium text-text-primary">
                                    <KeyRound className="h-4 w-4 text-accent-default" />
                                    {tr('auth.setup.password_security_title', 'Local security')}
                                </div>
                                {tr('auth.setup.password_security_desc', 'Your master password is used to protect local data and is not uploaded.')}
                            </div>
                            {error && <p className="text-fs-xs text-color-danger">{error}</p>}
                            <NavButtons
                                onBack={() => { setError(''); setStep('workspace'); }}
                                backLabel={tr('auth.setup.back', 'Back')}
                                nextLabel={tr('common.next', 'Next')}
                                onNext={handlePasswordNext}
                                nextDisabled={!canContinuePassword}
                            />
                        </div>
                        )
                    )}

                    {step === 'recovery' && (
                        <div className="space-y-5">
                            <div className="rounded-xl border border-amber-300/60 bg-amber-50 p-4">
                                <div className="mb-2 flex items-center gap-2 text-fs-sm font-semibold text-amber-900">
                                    <AlertTriangle className="h-4 w-4" />
                                    {tr('auth.setup.recovery_warning_title', 'Save this before continuing')}
                                </div>
                                <p className="text-fs-xs leading-5 text-amber-900/80">
                                    {tr('auth.setup.recovery_warning_desc', 'InsightCAP cannot show this phrase again after setup is complete.')}
                                </p>
                            </div>
                            <div className="rounded-xl border border-stroke-divider bg-surface-base p-5 font-mono text-fs-sm leading-7 text-text-primary shadow-inner">
                                {recoveryPhrase}
                            </div>
                            <Button variant="secondary" className="w-full" onClick={handleCopyRecovery}>
                                {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                                {copied ? tr('common.copied', 'Copied') : tr('common.copy', 'Copy')}
                            </Button>
                            <label className="flex cursor-pointer items-center gap-2 rounded-lg border border-stroke-divider bg-surface-base px-3 py-3">
                                <input
                                    type="checkbox"
                                    checked={recoveryConfirmed}
                                    onChange={e => setRecoveryConfirmed(e.target.checked)}
                                    className="h-4 w-4 rounded accent-accent-default"
                                />
                                <span className="text-fs-sm text-text-secondary">
                                    {tr('auth.setup.recovery_confirm', 'I have safely saved my recovery phrase')}
                                </span>
                            </label>
                            <div className="flex gap-3 pt-1">
                                <Button className="flex-1" disabled={!recoveryConfirmed} onClick={completeOnboarding}>
                                    {tr('auth.setup.finish', 'Finish Setup')} <ChevronRight className="h-4 w-4" />
                                </Button>
                            </div>
                        </div>
                    )}

                    {step === 'ai_provider' && (
                        <div className="space-y-5">
                            <div className="rounded-xl border border-stroke-divider bg-surface-base p-4 text-fs-sm leading-6 text-text-secondary">
                                {tr('auth.setup.ai_optional_hint', 'You can finish setup now. AI features will stay unavailable until you configure a provider.')}
                            </div>
                            <div>
                                <label htmlFor="setup-provider" className="mb-1.5 block text-fs-sm font-medium text-text-primary">
                                    {tr('settings.provider_type', 'Provider Type')}
                                </label>
                                <select
                                    id="setup-provider"
                                    value={setupProvider}
                                    onChange={e => {
                                        const provider = e.target.value;
                                        setSetupProvider(provider);
                                        setSetupModel(provider === 'ollama' ? 'qwen2.5:7b' : 'gpt-4o-mini');
                                    }}
                                    className="h-10 w-full rounded-lg border border-stroke-divider bg-surface-base px-3 text-fs-sm text-text-primary focus:outline-none focus:ring-1 focus:ring-stroke-focus"
                                >
                                    {Object.entries(providerLabels).map(([value, label]) => (
                                        <option key={value} value={value}>{label}</option>
                                    ))}
                                </select>
                            </div>
                            <Input
                                id="setup-model"
                                label={tr('settings.model_name', 'Model Name')}
                                value={setupModel}
                                onChange={e => setSetupModel(e.target.value)}
                                placeholder={setupProvider === 'ollama' ? 'qwen2.5:7b' : 'gpt-4o-mini'}
                            />
                            {setupProvider !== 'ollama' && (
                                <Input
                                    id="setup-api-key"
                                    label={tr('settings.provider_apikey', 'API Key')}
                                    type={showApiKey ? 'text' : 'password'}
                                    value={setupApiKey}
                                    onChange={e => setSetupApiKey(e.target.value)}
                                    placeholder="sk-..."
                                    rightElement={
                                        <button
                                            type="button"
                                            aria-label={showApiKey ? tr('auth.setup.hide_api_key', 'Hide API key') : tr('auth.setup.show_api_key', 'Show API key')}
                                            onClick={() => setShowApiKey(!showApiKey)}
                                            className="text-text-tertiary hover:text-text-primary"
                                        >
                                            {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                        </button>
                                    }
                                />
                            )}
                            {error && <p className="text-fs-xs text-color-danger">{error}</p>}
                            <div className="flex flex-col gap-3 pt-1 sm:flex-row">
                                <Button variant="secondary" className="sm:w-32" disabled={loading} onClick={() => { setError(''); setStep('password'); }}>
                                    <ChevronLeft className="h-4 w-4" /> {tr('common.back', 'Back')}
                                </Button>
                                <Button variant="secondary" className="flex-1" disabled={loading} onClick={handleSkipAiAndCreate}>
                                    {tr('auth.setup.skip_and_create', 'Skip and Create')}
                                </Button>
                                <Button className="flex-1" loading={loading} onClick={handleCreateRecoveryWithAi}>
                                    {loading ? tr('common.loading', 'Loading...') : tr('auth.setup.create_and_continue', 'Create and Continue')}
                                </Button>
                            </div>
                        </div>
                    )}
                </section>
            </div>
        </div>
    );
}

function HotkeyRow({ label, value }: { label: string; value: string }) {
    return (
        <div className="flex items-center justify-between gap-3">
            <span className="text-fs-sm text-text-secondary">{label}</span>
            <kbd className="rounded-md border border-white/10 bg-slate-900 px-2 py-1 font-mono text-fs-xs text-slate-100">
                {value}
            </kbd>
        </div>
    );
}

function NavButtons({
    onBack,
    onNext,
    backLabel,
    nextLabel,
    nextDisabled,
    loading,
}: {
    onBack: () => void;
    onNext: () => void;
    backLabel: string;
    nextLabel: string;
    nextDisabled?: boolean;
    loading?: boolean;
}) {
    return (
        <div className="flex gap-3 pt-1">
            <Button variant="secondary" className="w-32" disabled={loading} onClick={onBack}>
                <ChevronLeft className="h-4 w-4" /> {backLabel}
            </Button>
            <Button className="flex-1" loading={loading} disabled={nextDisabled} onClick={onNext}>
                {nextLabel} <ChevronRight className="h-4 w-4" />
            </Button>
        </div>
    );
}
