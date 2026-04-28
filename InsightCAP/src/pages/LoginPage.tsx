import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Lock } from 'lucide-react';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { invoke } from '@tauri-apps/api/core';

interface LoginPageProps {
    onLoginSuccess: () => void;
    kbPath: string;
}

interface LockStatus {
    isLocked: boolean;
    isPermanentlyLocked: boolean;
    remainingSecs?: number;
    failCount: number;
}

export function LoginPage({ onLoginSuccess, kbPath }: LoginPageProps) {
    const { t } = useTranslation();
    const [password, setPassword] = useState('');
    const [showPassword, setShowPassword] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');
    const [lockStatus, setLockStatus] = useState<LockStatus | null>(null);
    const [countdown, setCountdown] = useState(0);
    const [showRecovery, setShowRecovery] = useState(false);

    useEffect(() => {
        loadLockStatus();
    }, [kbPath]);

    useEffect(() => {
        if (countdown > 0) {
            const timer = setTimeout(() => setCountdown(c => c - 1), 1000);
            return () => clearTimeout(timer);
        }
    }, [countdown]);

    async function loadLockStatus() {
        try {
            const status = await invoke<LockStatus>('get_lock_status', { kbPath });
            setLockStatus(status);
            if (status.remainingSecs) {
                setCountdown(status.remainingSecs);
            }
        } catch { }
    }

    async function handleLogin() {
        if (!password) return;
        setError('');
        setLoading(true);
        try {
            await invoke('login', { payload: { password, kbPath } });
            onLoginSuccess();
        } catch (e) {
            const msg = String(e);
            if (msg.startsWith('LOCKED:')) {
                const secs = parseInt(msg.split(':')[1]);
                setCountdown(secs);
                setError(t('auth.login.locked', { seconds: secs }));
            } else if (msg === 'PERMANENTLY_LOCKED') {
                setError(t('auth.login.permanently_locked'));
            } else {
                setError(t('auth.login.wrong_password'));
            }
            await loadLockStatus();
        } finally {
            setLoading(false);
        }
    }

    const isLocked = countdown > 0 || lockStatus?.isPermanentlyLocked;

    return (
        <div className="flex h-screen w-screen items-center justify-center bg-surface-base">
            <div className="w-full max-w-sm rounded-2xl border border-stroke-divider p-8 bg-surface-card shadow-flyout">
                <div className="mb-6 text-center">
                    <div className="inline-flex h-12 w-12 items-center justify-center rounded-xl mb-3 bg-accent-light2">
                        <Lock className="h-6 w-6 text-accent-default" />
                    </div>
                    <h1 className="text-fs-xl font-bold text-text-primary">
                        {t('auth.login.title')}
                    </h1>
                </div>

                {!showRecovery ? (
                    <div className="space-y-4">
                        <Input
                            label={t('auth.login.password_label')}
                            type={showPassword ? 'text' : 'password'}
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            placeholder={t('auth.login.password_placeholder')}
                            disabled={isLocked}
                            error={error}
                            onKeyDown={e => e.key === 'Enter' && !isLocked && handleLogin()}
                            rightElement={
                                <button
                                    type="button"
                                    onClick={() => setShowPassword(!showPassword)}
                                    className="text-text-tertiary"
                                >
                                    {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                </button>
                            }
                        />

                        {countdown > 0 && (
                            <p className="text-fs-sm text-color-warning">
                                {t('auth.login.locked', { seconds: countdown })}
                            </p>
                        )}

                        <Button
                            className="w-full"
                            loading={loading}
                            disabled={isLocked || !password}
                            onClick={handleLogin}
                        >
                            {t('auth.login.submit')}
                        </Button>

                        <button
                            type="button"
                            className="w-full text-fs-xs text-center mt-2 text-text-tertiary"
                            onClick={() => setShowRecovery(true)}
                        >
                            {t('auth.login.forgot_password')}
                        </button>
                    </div>
                ) : (
                    <RecoveryForm kbPath={kbPath} onSuccess={onLoginSuccess} onBack={() => setShowRecovery(false)} />
                )}

                <div className="mt-10 pt-6 border-t border-stroke-divider/30 text-center space-y-1">
                    <p className="text-fs-xs font-medium text-text-tertiary">
                        {t('common.created_by')}
                    </p>
                    <p className="text-[10px] text-text-tertiary/50">
                        {t('common.copyright')}
                    </p>
                </div>
            </div>
        </div>
    );
}

function RecoveryForm({
    kbPath,
    onSuccess,
    onBack,
}: {
    kbPath: string;
    onSuccess: () => void;
    onBack: () => void;
}) {
    const { t } = useTranslation();
    const [mnemonic, setMnemonic] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    async function handleRecover() {
        setError('');
        setLoading(true);
        try {
            await invoke('recover_with_mnemonic', { payload: { mnemonic, newPassword, kbPath } });
            onSuccess();
        } catch {
            setError(t('auth.recovery.invalid_mnemonic'));
        } finally {
            setLoading(false);
        }
    }

    return (
        <div className="space-y-4">
            <h2 className="text-fs-sm font-semibold text-text-primary">
                {t('auth.recovery.title')}
            </h2>
            <div>
                <label className="mb-1.5 block text-fs-sm font-medium text-text-primary">
                    {t('auth.recovery.mnemonic_label')}
                </label>
                <textarea
                    value={mnemonic}
                    onChange={e => setMnemonic(e.target.value)}
                    rows={3}
                    className="w-full rounded-md border border-stroke-divider px-3 py-2 text-fs-sm font-mono bg-surface-base text-text-primary focus:outline-none focus:ring-1 focus:ring-stroke-focus"
                />
            </div>
            <Input
                label={t('auth.recovery.new_password_label')}
                type="password"
                value={newPassword}
                onChange={e => setNewPassword(e.target.value)}
                error={error}
            />
            <div className="flex gap-2">
                <Button variant="secondary" onClick={onBack}>{t('common.back')}</Button>
                <Button className="flex-1" loading={loading} onClick={handleRecover}>
                    {t('auth.recovery.submit')}
                </Button>
            </div>
        </div>
    );
}
