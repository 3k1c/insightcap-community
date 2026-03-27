import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, Eye, EyeOff, Copy, Check, ChevronRight, ChevronLeft } from 'lucide-react';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

type Step = 'workspace' | 'password' | 'recovery';

interface SetupPageProps {
    onComplete: () => void;
}

export function SetupPage({ onComplete }: SetupPageProps) {
    const { t } = useTranslation();
    const [step, setStep] = useState<Step>('workspace');
    const [workspacePath, setWorkspacePath] = useState('');
    const [password, setPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [showPassword, setShowPassword] = useState(false);
    const [showConfirmPassword, setShowConfirmPassword] = useState(false);
    const [recoveryPhrase, setRecoveryPhrase] = useState('');
    const [recoveryConfirmed, setRecoveryConfirmed] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [copied, setCopied] = useState(false);

    // 步驟指示器
    const steps: Step[] = ['workspace', 'password', 'recovery'];
    const stepIndex = steps.indexOf(step);

    const stepTitles: Record<Step, string> = {
        workspace: t('auth.setup.step1_title'),
        password: t('auth.setup.step2_title'),
        recovery: t('auth.setup.step3_title'),
    };

    async function handleBrowse() {
        try {
            const selected = await openDialog({ directory: true });
            if (selected) {
                setWorkspacePath(selected as string);
            }
        } catch {
            // 使用者取消選擇
        }
    }

    async function handleSetupSubmit() {
        setError('');

        if (!workspacePath.trim()) {
            setError('請輸入工作區路徑');
            return;
        }
        if (password.length < 8) {
            setError(t('auth.setup.password_too_short'));
            return;
        }
        if (password !== confirmPassword) {
            setError(t('auth.setup.password_mismatch'));
            return;
        }

        setLoading(true);
        try {
            const mnemonic = await invoke<string>('setup_auth', {
                payload: {
                    displayName: 'User',
                    password,
                    autoLogin: true,
                    kbPath: workspacePath,
                },
            });
            setRecoveryPhrase(mnemonic);
            setStep('recovery');
        } catch (e) {
            setError(String(e));
        } finally {
            setLoading(false);
        }
    }

    async function handleCopyRecovery() {
        try {
            await navigator.clipboard.writeText(recoveryPhrase);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch { }
    }

    async function handleFinish() {
        if (!recoveryConfirmed) return;
        try {
            await invoke('confirm_new_recovery', { kbPath: workspacePath });
            await invoke('restart_app');
        } catch {
            onComplete();
        }
    }

    return (
        <div
            className="flex h-screen w-screen items-center justify-center"
            style={{ background: 'var(--ic-bg-base)' }}
        >
            <div
                className="w-full max-w-md rounded-2xl border p-8"
                style={{
                    background: 'var(--ic-bg-elevated)',
                    borderColor: 'var(--ic-border-default)',
                    boxShadow: 'var(--ic-shadow-lg)',
                }}
            >
                {/* Logo / Header */}
                <div className="mb-6 text-center">
                    <div
                        className="inline-flex h-12 w-12 items-center justify-center rounded-xl mb-3"
                        style={{ background: 'var(--ic-accent-subtle)' }}
                    >
                        <span className="text-2xl font-bold" style={{ color: 'var(--ic-accent)' }}>⚡</span>
                    </div>
                    <h1 className="text-xl font-bold" style={{ color: 'var(--ic-text-primary)' }}>
                        {t('auth.setup.title')}
                    </h1>
                    <p className="mt-1 text-sm" style={{ color: 'var(--ic-text-muted)' }}>
                        {t('auth.setup.subtitle')}
                    </p>
                </div>

                {/* Progress Indicator */}
                <div className="flex items-center gap-2 mb-6">
                    {steps.map((s, i) => (
                        <React.Fragment key={s}>
                            <div
                                className="flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium transition-colors"
                                style={{
                                    background: i <= stepIndex ? 'var(--ic-accent)' : 'var(--ic-bg-sunken)',
                                    color: i <= stepIndex ? 'var(--ic-text-inverse)' : 'var(--ic-text-muted)',
                                }}
                            >
                                {i + 1}
                            </div>
                            {i < steps.length - 1 && (
                                <div
                                    className="flex-1 h-px"
                                    style={{ background: i < stepIndex ? 'var(--ic-accent)' : 'var(--ic-border-default)' }}
                                />
                            )}
                        </React.Fragment>
                    ))}
                </div>

                <h2 className="text-base font-semibold mb-4" style={{ color: 'var(--ic-text-primary)' }}>
                    {stepTitles[step]}
                </h2>

                {/* Step 1: Workspace */}
                {step === 'workspace' && (
                    <div className="space-y-4">
                        <div>
                            <label className="mb-1.5 block text-sm font-medium" style={{ color: 'var(--ic-text-primary)' }}>
                                {t('auth.setup.workspace_label')}
                            </label>
                            <div className="flex gap-2">
                                <input
                                    type="text"
                                    value={workspacePath}
                                    onChange={e => setWorkspacePath(e.target.value)}
                                    placeholder={t('auth.setup.workspace_placeholder')}
                                    className="flex-1 h-9 rounded-md border px-3 text-sm focus:outline-none focus:ring-1"
                                    style={{
                                        background: 'var(--ic-bg-base)',
                                        borderColor: 'var(--ic-border-default)',
                                        color: 'var(--ic-text-primary)',
                                    }}
                                />
                                <Button variant="secondary" size="icon" onClick={handleBrowse}>
                                    <FolderOpen className="h-4 w-4" />
                                </Button>
                            </div>
                            <p className="mt-1 text-xs" style={{ color: 'var(--ic-text-muted)' }}>
                                {t('auth.setup.workspace_hint')}
                            </p>
                        </div>
                        {error && <p className="text-xs" style={{ color: 'var(--ic-danger)' }}>{error}</p>}
                        <Button
                            className="w-full"
                            onClick={() => { setError(''); if (workspacePath.trim()) setStep('password'); else setError('請輸入工作區路徑'); }}
                        >
                            {t('common.next')} <ChevronRight className="h-4 w-4" />
                        </Button>
                    </div>
                )}

                {/* Step 2: Password */}
                {step === 'password' && (
                    <div className="space-y-4">
                        <Input
                            label={t('auth.setup.password_label')}
                            type={showPassword ? 'text' : 'password'}
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            placeholder={t('auth.setup.password_placeholder')}
                            rightElement={
                                <button
                                    type="button"
                                    onClick={() => setShowPassword(!showPassword)}
                                    style={{ color: 'var(--ic-text-muted)' }}
                                >
                                    {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                </button>
                            }
                        />
                        <Input
                            label={t('auth.setup.password_confirm_label')}
                            type={showConfirmPassword ? 'text' : 'password'}
                            value={confirmPassword}
                            onChange={e => setConfirmPassword(e.target.value)}
                            placeholder={t('auth.setup.password_confirm_placeholder')}
                            error={error}
                            rightElement={
                                <button
                                    type="button"
                                    onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                                    style={{ color: 'var(--ic-text-muted)' }}
                                >
                                    {showConfirmPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                </button>
                            }
                        />
                        <div className="flex gap-2">
                            <Button variant="secondary" onClick={() => setStep('workspace')}>
                                <ChevronLeft className="h-4 w-4" /> {t('common.back')}
                            </Button>
                            <Button className="flex-1" loading={loading} onClick={handleSetupSubmit}>
                                {t('common.next')} <ChevronRight className="h-4 w-4" />
                            </Button>
                        </div>
                    </div>
                )}

                {/* Step 3: Recovery Phrase */}
                {step === 'recovery' && (
                    <div className="space-y-4">
                        <p className="text-sm" style={{ color: 'var(--ic-text-secondary)' }}>
                            {t('auth.setup.recovery_hint')}
                        </p>
                        <div
                            className="rounded-lg p-4 font-mono text-xs leading-relaxed select-all"
                            style={{
                                background: 'var(--ic-bg-sunken)',
                                color: 'var(--ic-text-primary)',
                                wordBreak: 'break-all',
                            }}
                        >
                            {recoveryPhrase}
                        </div>
                        <Button variant="secondary" className="w-full" onClick={handleCopyRecovery}>
                            {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                            {copied ? t('common.copied') : t('common.copy')}
                        </Button>
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                checked={recoveryConfirmed}
                                onChange={e => setRecoveryConfirmed(e.target.checked)}
                                className="h-4 w-4 rounded"
                                style={{ accentColor: 'var(--ic-accent)' }}
                            />
                            <span className="text-sm" style={{ color: 'var(--ic-text-secondary)' }}>
                                {t('auth.setup.recovery_confirm')}
                            </span>
                        </label>
                        <Button
                            className="w-full"
                            disabled={!recoveryConfirmed}
                            onClick={handleFinish}
                        >
                            {t('auth.setup.finish')}
                        </Button>
                    </div>
                )}
            </div>
        </div>
    );
}
