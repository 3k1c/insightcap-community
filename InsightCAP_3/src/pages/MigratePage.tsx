import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, HardDrive } from 'lucide-react';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { invoke } from '@tauri-apps/api/core';

interface MigratePageProps {
    kbPath: string;
    onUnlockSuccess: () => void;
}

type UnlockMethod = 'password' | 'mnemonic';

// 恢復碼路徑的第二步：顯示新日常恢復碼並確認後重啟
function NewRecoveryStep({
    newMnemonic,
    onConfirm,
}: {
    newMnemonic: string;
    onConfirm: () => void;
}) {
    const { t } = useTranslation();
    const [copied, setCopied] = useState(false);
    const [confirmed, setConfirmed] = useState(false);

    async function handleCopy() {
        await navigator.clipboard.writeText(newMnemonic);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    }

    return (
        <div className="space-y-4">
            <p className="text-fs-sm text-text-secondary">{t('auth.migrate.new_recovery_hint')}</p>
            <div className="rounded-lg border border-stroke-divider bg-surface-base p-3">
                <p className="font-mono text-fs-sm text-text-primary break-all leading-relaxed">
                    {newMnemonic}
                </p>
            </div>
            <Button variant="secondary" className="w-full" onClick={handleCopy}>
                {copied ? t('common.copied') : t('common.copy')}
            </Button>
            <label className="flex items-center gap-2 cursor-pointer">
                <input
                    type="checkbox"
                    checked={confirmed}
                    onChange={e => setConfirmed(e.target.checked)}
                    className="accent-accent-default"
                />
                <span className="text-fs-sm text-text-secondary">
                    {t('auth.migrate.new_recovery_confirm')}
                </span>
            </label>
            <Button className="w-full" disabled={!confirmed} onClick={onConfirm}>
                {t('auth.migrate.restart')}
            </Button>
        </div>
    );
}

export function MigratePage({ kbPath, onUnlockSuccess }: MigratePageProps) {
    const { t } = useTranslation();
    const [method, setMethod] = useState<UnlockMethod>('password');

    // 密碼路徑
    const [password, setPassword] = useState('');
    const [showPassword, setShowPassword] = useState(false);

    // 恢復碼路徑
    const [mnemonic, setMnemonic] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [showNewPassword, setShowNewPassword] = useState(false);
    const [newMnemonic, setNewMnemonic] = useState('');

    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    async function handlePasswordUnlock() {
        if (!password) return;
        setError('');
        setLoading(true);
        try {
            await invoke('unlock_migrated_with_password', { kbPath, password });
            // Keychain 已寫入，重新走 initApp 流程（try_auto_login 會成功）
            onUnlockSuccess();
        } catch (e) {
            const msg = String(e);
            if (msg === 'WRONG_PASSWORD') {
                setError(t('auth.migrate.wrong_password'));
            } else {
                setError(msg);
            }
        } finally {
            setLoading(false);
        }
    }

    async function handleMnemonicUnlock() {
        if (!mnemonic || !newPassword) return;
        setError('');
        setLoading(true);
        try {
            const result = await invoke<string>('unlock_migrated_with_mnemonic', {
                kbPath,
                mnemonic,
                newPassword,
            });
            setNewMnemonic(result);
        } catch (e) {
            const msg = String(e);
            if (msg === 'INVALID_MNEMONIC') {
                setError(t('auth.migrate.invalid_mnemonic'));
            } else {
                setError(msg);
            }
        } finally {
            setLoading(false);
        }
    }

    async function handleRestart() {
        await invoke('restart_app');
    }

    // 恢復碼路徑第二步：顯示新日常恢復碼
    if (newMnemonic) {
        return (
            <div className="flex h-screen w-screen items-center justify-center bg-surface-base">
                <div className="w-full max-w-sm rounded-2xl border border-stroke-divider p-8 bg-surface-card shadow-flyout">
                    <div className="mb-6 text-center">
                        <div className="inline-flex h-12 w-12 items-center justify-center rounded-xl mb-3 bg-accent-light2">
                            <HardDrive className="h-6 w-6 text-accent-default" />
                        </div>
                        <h1 className="text-fs-xl font-bold text-text-primary">
                            {t('auth.migrate.new_recovery_title')}
                        </h1>
                    </div>
                    <NewRecoveryStep newMnemonic={newMnemonic} onConfirm={handleRestart} />
                </div>
            </div>
        );
    }

    return (
        <div className="flex h-screen w-screen items-center justify-center bg-surface-base">
            <div className="w-full max-w-sm rounded-2xl border border-stroke-divider p-8 bg-surface-card shadow-flyout">
                {/* Icon + 標題 */}
                <div className="mb-6 text-center">
                    <div className="inline-flex h-12 w-12 items-center justify-center rounded-xl mb-3 bg-accent-light2">
                        <HardDrive className="h-6 w-6 text-accent-default" />
                    </div>
                    <h1 className="text-fs-xl font-bold text-text-primary">
                        {t('auth.migrate.title')}
                    </h1>
                    <p className="mt-1 text-fs-sm text-text-secondary">
                        {t('auth.migrate.subtitle')}
                    </p>
                </div>

                {/* 解鎖方式選擇 */}
                <div className="mb-4 space-y-2">
                    <label className="flex items-center gap-2 cursor-pointer">
                        <input
                            type="radio"
                            name="unlockMethod"
                            value="password"
                            checked={method === 'password'}
                            onChange={() => { setMethod('password'); setError(''); }}
                            className="accent-accent-default"
                        />
                        <span className="text-fs-sm text-text-primary">
                            {t('auth.migrate.method_password')}
                        </span>
                    </label>
                    <label className="flex items-center gap-2 cursor-pointer">
                        <input
                            type="radio"
                            name="unlockMethod"
                            value="mnemonic"
                            checked={method === 'mnemonic'}
                            onChange={() => { setMethod('mnemonic'); setError(''); }}
                            className="accent-accent-default"
                        />
                        <span className="text-fs-sm text-text-primary">
                            {t('auth.migrate.method_mnemonic')}
                        </span>
                    </label>
                </div>

                {/* 密碼路徑 */}
                {method === 'password' && (
                    <div className="space-y-4">
                        <Input
                            label={t('auth.migrate.password_label')}
                            type={showPassword ? 'text' : 'password'}
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            placeholder={t('auth.migrate.password_placeholder')}
                            error={error}
                            onKeyDown={e => e.key === 'Enter' && handlePasswordUnlock()}
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
                        <Button
                            className="w-full"
                            loading={loading}
                            disabled={!password}
                            onClick={handlePasswordUnlock}
                        >
                            {t('auth.migrate.unlock')}
                        </Button>
                    </div>
                )}

                {/* 恢復碼路徑 */}
                {method === 'mnemonic' && (
                    <div className="space-y-4">
                        <div>
                            <label className="mb-1.5 block text-fs-sm font-medium text-text-primary">
                                {t('auth.migrate.mnemonic_label')}
                            </label>
                            <textarea
                                value={mnemonic}
                                onChange={e => setMnemonic(e.target.value)}
                                rows={3}
                                className="w-full rounded-md border border-stroke-divider px-3 py-2 text-fs-sm font-mono bg-surface-base text-text-primary focus:outline-none focus:ring-1 focus:ring-stroke-focus"
                                placeholder={t('auth.migrate.mnemonic_placeholder')}
                            />
                        </div>
                        <Input
                            label={t('auth.migrate.new_password_label')}
                            type={showNewPassword ? 'text' : 'password'}
                            value={newPassword}
                            onChange={e => setNewPassword(e.target.value)}
                            placeholder={t('auth.migrate.new_password_placeholder')}
                            error={error}
                            rightElement={
                                <button
                                    type="button"
                                    onClick={() => setShowNewPassword(!showNewPassword)}
                                    className="text-text-tertiary"
                                >
                                    {showNewPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                </button>
                            }
                        />
                        <Button
                            className="w-full"
                            loading={loading}
                            disabled={!mnemonic || !newPassword}
                            onClick={handleMnemonicUnlock}
                        >
                            {t('auth.migrate.unlock')}
                        </Button>
                    </div>
                )}
            </div>
        </div>
    );
}
