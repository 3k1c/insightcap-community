import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { ExternalKnowledgeBase, ExternalKbLoadResult } from '../lib/types';
import { useThemeStore, type Theme } from '../stores/themeStore';
import { useLanguageStore } from '../stores/languageStore';
import type { Language } from '../i18n';
import {
    Settings2, Server, Sparkles, BookOpen, Ellipsis,
    Plus, Trash2, Eye, EyeOff, Database, Zap,
    RefreshCw, Download, Upload, AlertTriangle, Wrench,
} from 'lucide-react';
import { useT } from '../hooks/useT';
import { save } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';

// ── Types matching Rust AllSettings ──────────────────────

interface ModelSettings {
    provider: string;
    model: string;
    apiKey?: string;
    baseUrl?: string;
}

interface ProviderProfileData {
    id: string;
    name: string;
    provider: string;
    baseUrl?: string;
    apiKey?: string;
}

interface AllSettings {
    general: {
        launchAtStartup: boolean;
        minimizeToTray: boolean;
        language: string;
    };
    aiModels: {
        chatLlm: ModelSettings;
        contentProcessorLlm: ModelSettings;
        visionModel: ModelSettings;
        embeddingModel: ModelSettings;
        summaryModel?: string;
        providerProfiles: ProviderProfileData[];
    };
    knowledge: {
        kbPath: string;
        autoClassifyEnabled: boolean;
        autoSpaceMode: string;
    };
    hotkeys: {
        captureClipboard: string;
        quickInput: string;
    };
    autoCleanup: {
        enabled: boolean;
        retentionDays: number;
    };
    webSearch: {
        enabled: boolean;
        provider: string;
        apiKey: string;
    };
    editor: {
        defaultFont: string;
        defaultFontSize: string;
        defaultLineSpacing: string;
        defaultExportFormat: string;
        exportSubdir: string;
    };
    bilibiliSessdata?: string;
    chatPromptInstruction: string;
}

type SettingsTab = 'general' | 'provider' | 'ai' | 'knowledge' | 'other';

// These will be translated in the component
const TABS: { id: SettingsTab; labelKey: string; icon: React.FC<{ className?: string }> }[] = [
    { id: 'general', labelKey: 'settings.general', icon: Settings2 },
    { id: 'provider', labelKey: 'settings.ai_models', icon: Server },
    { id: 'ai', labelKey: 'settings.ai_section', icon: Sparkles },
    { id: 'knowledge', labelKey: 'settings.knowledge', icon: BookOpen },
    { id: 'other', labelKey: 'settings.other', icon: Ellipsis },
];

const THEMES: { value: Theme }[] = [
    { value: 'frost' },
    { value: 'void' },
    { value: 'warm' },
    { value: 'sage' },
];



const PROVIDER_OPTIONS: { value: string; label: string }[] = [
    { value: 'ollama', label: 'Ollama' },
    { value: 'openai', label: 'OpenAI' },
    { value: 'anthropic', label: 'Anthropic' },
    { value: 'google', label: 'Google' },
    { value: 'xai', label: 'xAI' },
    { value: 'openrouter', label: 'OpenRouter' },
];

// ── Helper Components ────────────────────────────────────

const SectionCard: React.FC<{ title: string; desc?: string; children: React.ReactNode; action?: React.ReactNode }> = ({ title, desc, children, action }) => (
    <div className="bg-surface-layer border border-stroke-divider rounded-xl p-6 mb-6 shadow-sm">
        <div className="flex items-start justify-between mb-4">
            <div>
                <h4 className="font-semibold text-text-primary">{title}</h4>
                {desc && <p className="text-fs-sm text-text-secondary mt-1">{desc}</p>}
            </div>
            {action}
        </div>
        <div className="space-y-4">{children}</div>
    </div>
);

const SettingRow: React.FC<{ label: string; desc?: string; children: React.ReactNode }> = ({ label, desc, children }) => (
    <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
            <div className="text-fs-sm font-medium text-text-primary">{label}</div>
            {desc && <div className="text-fs-xs text-text-tertiary mt-0.5">{desc}</div>}
        </div>
        <div className="shrink-0">{children}</div>
    </div>
);

const Toggle: React.FC<{ checked: boolean; onChange: (v: boolean) => void }> = ({ checked, onChange }) => (
    <button
        type="button"
        onClick={() => onChange(!checked)}
        className={`relative w-10 h-5 rounded-full transition-colors ${checked ? 'bg-accent-default' : 'bg-stroke-divider'}`}
    >
        <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${checked ? 'translate-x-5' : ''}`} />
    </button>
);

const SelectField: React.FC<{ value: string; onChange: (v: string) => void; options: { value: string; label: string }[]; className?: string }> = ({ value, onChange, options, className }) => (
    <select
        value={value}
        onChange={e => onChange(e.target.value)}
        className={`bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary focus:outline-none focus:ring-1 focus:ring-accent-default ${className ?? ''}`}
    >
        {options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
);

const InputField: React.FC<{ value: string; onChange: (v: string) => void; placeholder?: string; type?: string; className?: string }> = ({ value, onChange, placeholder, type = 'text', className }) => (
    <input
        type={type}
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        className={`bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary placeholder:text-text-tertiary focus:outline-none focus:ring-1 focus:ring-accent-default ${className ?? ''}`}
    />
);

// ── Main Component ───────────────────────────────────────

export const SettingsPage: React.FC = () => {
    const t = useT();
    const [activeTab, setActiveTab] = useState<SettingsTab>('general');
    const [settings, setSettings] = useState<AllSettings | null>(null);
    const [saving, setSaving] = useState(false);
    const [externalKbs, setExternalKbs] = useState<ExternalKnowledgeBase[]>([]);
    const [kbLoading, setKbLoading] = useState(false);

    // Provider edit state
    const [editingProfile, setEditingProfile] = useState<ProviderProfileData | null>(null);
    const [showApiKeys, setShowApiKeys] = useState<Record<string, boolean>>({});
    const [bilibiliLoggingIn, setBilibiliLoggingIn] = useState(false);

    // Export modal state
    const [exportModal, setExportModal] = useState<{
        open: boolean;
        step: 'confirm' | 'mnemonic';
        mnemonic: string;
        confirmed: boolean;
        destPath: string;
        loading: boolean;
    }>({ open: false, step: 'confirm', mnemonic: '', confirmed: false, destPath: '', loading: false });

    // Import modal state
    const [importModal, setImportModal] = useState<{
        open: boolean;
        step: 'input' | 'new_recovery';
        srcPath: string;
        mnemonic: string;
        newPassword: string;
        newMnemonic: string;
        confirmed: boolean;
        loading: boolean;
        error: string;
    }>({ open: false, step: 'input', srcPath: '', mnemonic: '', newPassword: '', newMnemonic: '', confirmed: false, loading: false, error: '' });

    const { theme, setTheme } = useThemeStore();
    const { language, setLanguage } = useLanguageStore();

    // ── Load settings ────────────────────────────────────

    useEffect(() => {
        loadSettings();
    }, []);

    useEffect(() => {
        if (activeTab === 'knowledge' || activeTab === 'other') {
            loadExternalKbs();
        }
    }, [activeTab]);

    const loadSettings = async () => {
        try {
            const s = await invoke<AllSettings>('get_settings');
            setSettings(s);
        } catch (e) {
            console.error('Failed to load settings', e);
            toast.error('無法載入設定');
        }
    };

    const saveSettings = async (updated?: AllSettings) => {
        const target = updated ?? settings;
        if (!target) return;
        setSaving(true);
        try {
            await invoke('save_settings', { settings: target });
            setSettings(target);
            toast.success('設定已儲存');
        } catch (e: any) {
            toast.error(`儲存失敗: ${e.toString()}`);
        } finally {
            setSaving(false);
        }
    };

    const updateSettings = (updater: (draft: AllSettings) => void) => {
        if (!settings) return;
        const copy = JSON.parse(JSON.stringify(settings)) as AllSettings;
        updater(copy);
        setSettings(copy);
    };

    const handleBilibiliLogin = async () => {
        setBilibiliLoggingIn(true);
        const tid = toast.loading(t('settings.bilibili_logging_in'));
        try {
            const sessdata = await invoke<string>('open_bilibili_login');
            updateSettings(s => { s.bilibiliSessdata = sessdata; });
            const copy = JSON.parse(JSON.stringify(settings)) as AllSettings;
            copy.bilibiliSessdata = sessdata;
            await invoke('save_settings', { settings: copy });
            setSettings(copy);
            toast.success(t('settings.bilibili_login_success'), { id: tid });
        } catch (e: any) {
            toast.error(e.toString(), { id: tid });
        } finally {
            setBilibiliLoggingIn(false);
        }
    };

    // ── External KB handlers ─────────────────────────────

    const loadExternalKbs = async () => {
        try {
            const kbs = await invoke<ExternalKnowledgeBase[]>('get_external_kbs');
            setExternalKbs(kbs);
        } catch (error) {
            console.error('Failed to load DBs', error);
        }
    };

    const handleAddKb = async () => {
        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'SQLite Database', extensions: ['db', 'sqlite'] }]
            });
            if (selected && typeof selected === 'string') {
                setKbLoading(true);
                const toastId = toast.loading('正在驗證與掛載外部知識庫...');
                try {
                    const result = await invoke<ExternalKbLoadResult>('load_external_kb', { dbPath: selected });
                    if (result.success) {
                        toast.success('外部知識庫掛載成功！', { id: toastId });
                        loadExternalKbs();
                    } else {
                        toast.error(`掛載失敗: ${result.reason}`, { id: toastId });
                    }
                } catch (e: any) {
                    toast.error(`發生錯誤: ${e.toString()}`, { id: toastId });
                }
            }
        } catch (error) {
            console.error('Add KB failed', error);
            toast.error('開啟檔案對話框失敗');
        } finally {
            setKbLoading(false);
        }
    };

    const handleRemoveKb = async (id: string, name: string) => {
        if (!window.confirm(`確定要移除外部知識庫 "${name}" 的連線嗎？`)) return;
        try {
            await invoke('remove_external_kb', { id });
            toast.success('已移除知識庫連線');
            loadExternalKbs();
        } catch (error) {
            console.error('Remove KB failed', error);
            toast.error('移除失敗');
        }
    };

    // ── Provider profile handlers ────────────────────────

    const handleSaveProfile = () => {
        if (!editingProfile || !settings) return;
        const profiles = [...settings.aiModels.providerProfiles];
        const idx = profiles.findIndex(p => p.id === editingProfile.id);
        if (idx >= 0) {
            profiles[idx] = editingProfile;
        } else {
            profiles.push(editingProfile);
        }
        const updated = { ...settings, aiModels: { ...settings.aiModels, providerProfiles: profiles } };
        saveSettings(updated);
        setEditingProfile(null);
    };

    const handleDeleteProfile = (id: string) => {
        if (!settings) return;
        if (!window.confirm('確定要刪除此 Provider？')) return;
        const profiles = settings.aiModels.providerProfiles.filter(p => p.id !== id);
        const updated = { ...settings, aiModels: { ...settings.aiModels, providerProfiles: profiles } };
        saveSettings(updated);
    };

    const handleTestConnection = async (profile: ProviderProfileData) => {
        const tid = toast.loading(t('common.loading'));
        try {
            if (profile.provider === 'ollama') {
                await invoke('test_ollama', { baseUrl: profile.baseUrl ?? 'http://localhost:11434' });
            } else {
                await invoke('test_provider_connection', {
                    provider: profile.provider,
                    baseUrl: profile.baseUrl ?? '',
                    apiKey: profile.apiKey ?? '',
                });
            }
            toast.success(t('common.success'), { id: tid });
        } catch (e: any) {
            toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
        }
    };

    // ── Tab: 一般設定 ────────────────────────────────────

    const renderGeneral = () => {
        if (!settings) return null;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.general_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.general_desc')}</p>
                </div>

                <SectionCard title={t('settings.appearance_section')}>
                    <SettingRow label={t('language.label')} desc={t('language.desc')}>
                        <SelectField
                            value={language}
                            onChange={v => setLanguage(v as Language)}
                            options={[
                                { value: 'zh-TW', label: t('language.zh-TW') },
                                { value: 'zh-CN', label: t('language.zh-CN') },
                                { value: 'en', label: t('language.en') },
                            ]}
                        />
                    </SettingRow>
                    <SettingRow label={t('theme.label')}>
                        <div className="flex gap-2">
                            {THEMES.map(themeOption => (
                                <button
                                    key={themeOption.value}
                                    onClick={() => setTheme(themeOption.value)}
                                    className={`px-3 py-1.5 rounded-lg text-fs-xs transition-colors border ${theme === themeOption.value
                                        ? 'border-accent-default bg-accent-default/10 text-accent-default font-medium'
                                        : 'border-stroke-divider text-text-secondary hover:bg-surface-subtle'
                                        }`}
                                    title={t(`theme.${themeOption.value}_desc` as any)}
                                >
                                    {t(`theme.${themeOption.value}` as any)}
                                </button>
                            ))}
                        </div>
                    </SettingRow>
                </SectionCard>

                <SectionCard title={t('settings.system_behavior')}>
                    <SettingRow label={t('settings.launch_at_startup')} desc={t('settings.launch_at_startup_desc')}>
                        <Toggle
                            checked={settings?.general.launchAtStartup ?? false}
                            onChange={v => { updateSettings(s => { s.general.launchAtStartup = v; }); }}
                        />
                    </SettingRow>
                    <SettingRow label={t('settings.minimize_to_tray')} desc={t('settings.minimize_to_tray_desc')}>
                        <Toggle
                            checked={settings?.general.minimizeToTray ?? true}
                            onChange={v => { updateSettings(s => { s.general.minimizeToTray = v; }); }}
                        />
                    </SettingRow>
                </SectionCard>

                <SectionCard title={t('settings.hotkeys')}>
                    <SettingRow label={t('settings.capture_clipboard')} desc={t('settings.global_hotkey')}>
                        <InputField value={settings.hotkeys.captureClipboard} onChange={v => updateSettings(s => { s.hotkeys.captureClipboard = v; })} className="w-40" />
                    </SettingRow>
                    <SettingRow label={t('settings.quick_input')} desc={t('settings.global_hotkey')}>
                        <InputField value={settings.hotkeys.quickInput} onChange={v => updateSettings(s => { s.hotkeys.quickInput = v; })} className="w-40" />
                    </SettingRow>
                </SectionCard>

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? t('common.saving') : t('common.save')}
                    </button>
                </div>
            </div>
        );
    };

    // ── Tab: LLM Provider ────────────────────────────────────────

    const renderProvider = () => {
        if (!settings) return null;
        const ai = settings.aiModels;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.ai_models')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.general_desc')}</p>
                </div>

                <SectionCard
                    title={t('settings.provider_management_title')}
                    desc={t('settings.provider_management_desc')}
                    action={
                        <button
                            onClick={() => setEditingProfile({ id: crypto.randomUUID(), name: '', provider: 'ollama', baseUrl: 'http://localhost:11434', apiKey: '' })}
                            className="flex items-center gap-1.5 bg-accent-default text-white px-4 py-2 rounded-xl text-fs-sm hover:bg-accent-light1 transition-all shadow-md active:scale-95"
                        >
                            <Plus className="w-4 h-4" /> {t('common.add')}
                        </button>
                    }
                >
                    {(settings?.aiModels.providerProfiles ?? []).length === 0 && !editingProfile && (
                        <div className="text-center py-8 text-text-tertiary">
                            <Server className="w-7 h-7 mx-auto mb-2 opacity-50" />
                            <p className="text-fs-sm">{t('settings.provider_empty')}</p>
                        </div>
                    )}

                    {(settings?.aiModels.providerProfiles ?? []).map(p => (
                        <div key={p.id} className="flex items-center justify-between gap-3 bg-surface-base rounded-lg px-4 py-3 border border-stroke-divider">
                            <div className="min-w-0">
                                <div className="font-medium text-text-primary text-fs-sm">{p.name || t('settings.provider_unnamed')}</div>
                                <div className="text-fs-xs text-text-tertiary mt-0.5">
                                    {PROVIDER_OPTIONS.find(o => o.value === p.provider)?.label ?? p.provider}
                                    {p.baseUrl ? ` · ${p.baseUrl}` : ''}
                                </div>
                            </div>
                            <div className="flex items-center gap-1">
                                <button onClick={() => handleTestConnection(p)} className="p-1.5 text-text-tertiary hover:text-accent-default rounded-md transition-colors" title={t('common.test')}>
                                    <Zap className="w-4 h-4" />
                                </button>
                                <button onClick={() => setEditingProfile({ ...p })} className="p-1.5 text-text-tertiary hover:text-accent-default rounded-md transition-colors" title={t('common.edit')}>
                                    <Settings2 className="w-4 h-4" />
                                </button>
                                <button onClick={() => handleDeleteProfile(p.id)} className="p-1.5 text-text-tertiary hover:text-red-500 rounded-md transition-colors" title={t('common.delete')}>
                                    <Trash2 className="w-4 h-4" />
                                </button>
                            </div>
                        </div>
                    ))}

                    {editingProfile && (
                        <div className="bg-surface-base rounded-lg p-4 border border-accent-default/30 space-y-3">
                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_name')}</label>
                                    <InputField value={editingProfile.name} onChange={v => setEditingProfile({ ...editingProfile, name: v })} placeholder="例: My OpenAI" className="w-full" />
                                </div>
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_type')}</label>
                                    <SelectField value={editingProfile.provider} onChange={v => setEditingProfile({ ...editingProfile, provider: v })} options={PROVIDER_OPTIONS} className="w-full" />
                                </div>
                            </div>
                            <div>
                                <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_base_url')}</label>
                                <InputField value={editingProfile.baseUrl ?? ''} onChange={v => setEditingProfile({ ...editingProfile, baseUrl: v })} placeholder="http://localhost:11434" className="w-full" />
                            </div>
                            {editingProfile.provider !== 'ollama' && (
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_apikey')}</label>
                                    <div className="relative">
                                        <InputField
                                            value={editingProfile.apiKey ?? ''}
                                            onChange={v => setEditingProfile({ ...editingProfile, apiKey: v })}
                                            placeholder="sk-..."
                                            type={showApiKeys[editingProfile.id] ? 'text' : 'password'}
                                            className="w-full pr-10"
                                        />
                                        <button
                                            type="button"
                                            onClick={() => setShowApiKeys(p => ({ ...p, [editingProfile.id]: !p[editingProfile.id] }))}
                                            className="absolute right-2 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
                                        >
                                            {showApiKeys[editingProfile.id] ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                        </button>
                                    </div>
                                </div>
                            )}
                            <div className="flex justify-end gap-2 pt-1">
                                <button onClick={() => setEditingProfile(null)} className="px-3 py-1.5 rounded-lg text-fs-sm text-text-secondary hover:bg-surface-subtle transition-colors">{t('common.cancel')}</button>
                                <button onClick={() => handleTestConnection(editingProfile)} className="px-3 py-1.5 rounded-lg text-fs-sm text-text-secondary hover:bg-surface-subtle transition-colors">{t('common.test')}</button>
                                <button onClick={handleSaveProfile} className="bg-accent-default text-white px-4 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors">{t('common.save')}</button>
                            </div>
                        </div>
                    )}
                </SectionCard>

                <SectionCard title={t('settings.model_config_title')} desc={t('settings.model_config_desc')}>
                    {renderModelField(t('settings.model_chat'), t('settings.model_chat_desc'), ai.chatLlm, m => updateSettings(s => { s.aiModels.chatLlm = m; }))}
                    {renderModelField(t('settings.model_processor'), t('settings.model_processor_desc'), ai.contentProcessorLlm, m => updateSettings(s => { s.aiModels.contentProcessorLlm = m; }))}
                    {renderModelField(t('settings.model_vision'), t('settings.model_vision_desc'), ai.visionModel, m => updateSettings(s => { s.aiModels.visionModel = m; }))}
                    {renderModelField(t('settings.model_embedding'), t('settings.model_embedding_desc'), ai.embeddingModel, m => updateSettings(s => { s.aiModels.embeddingModel = m; }))}
                </SectionCard>

                <SectionCard title={t('settings.model_summary_section')}>
                    <SettingRow label={t('settings.model_summary')} desc={t('settings.model_summary_desc')}>
                        <InputField
                            value={ai.summaryModel ?? 'follow_chat'}
                            onChange={v => updateSettings(s => { s.aiModels.summaryModel = v; })}
                            className="w-48"
                        />
                    </SettingRow>
                </SectionCard>

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? t('common.saving') : t('common.save')}
                    </button>
                </div>
            </div>
        );
    };
    // ── Tab: AI 設置 ─────────────────────────────────────



    const renderModelField = (label: string, desc: string, model: ModelSettings, onChange: (m: ModelSettings) => void) => (
        <div className="bg-surface-base rounded-lg px-4 py-3 border border-stroke-divider space-y-2">
            <div className="text-fs-sm font-medium text-text-primary">{label}</div>
            <div className="text-fs-xs text-text-tertiary">{desc}</div>
            <div className="grid grid-cols-2 gap-3 pt-1">
                <div>
                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_type')}</label>
                    <SelectField value={model.provider} onChange={v => onChange({ ...model, provider: v })} options={[...PROVIDER_OPTIONS, { value: 'local', label: t('settings.local_provider') }]} className="w-full" />
                </div>
                <div>
                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.model_name')}</label>
                    <InputField value={model.model} onChange={v => onChange({ ...model, model: v })} placeholder={t('settings.model_name')} className="w-full" />
                </div>
            </div>
        </div>
    );

    const renderAI = () => {
        if (!settings) return null;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.ai_section_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.ai_section_desc')}</p>
                </div>

                <SectionCard title={t('settings.ai_prompt_instruction_title')} desc={t('settings.ai_prompt_instruction_desc')}>
                    <textarea
                        value={settings.chatPromptInstruction}
                        onChange={e => updateSettings(s => { s.chatPromptInstruction = e.target.value; })}
                        placeholder={t('settings.ai_prompt_instruction_placeholder')}
                        rows={4}
                        className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-none focus:outline-none focus:ring-1 focus:ring-accent-default"
                    />
                </SectionCard>

                <SectionCard title={t('settings.web_search_title')}>
                    <SettingRow label={t('settings.web_search_enable')}>
                        <Toggle checked={settings.webSearch.enabled} onChange={v => updateSettings(s => { s.webSearch.enabled = v; })} />
                    </SettingRow>
                    {settings.webSearch.enabled && (
                        <>
                            <SettingRow label={t('settings.web_search_provider')}>
                                <SelectField
                                    value={settings.webSearch.provider}
                                    onChange={v => updateSettings(s => { s.webSearch.provider = v; })}
                                    options={[{ value: 'tavily', label: 'Tavily' }, { value: 'serper', label: 'Serper' }]}
                                />
                            </SettingRow>
                            <SettingRow label={t('settings.web_search_apikey')}>
                                <div className="relative">
                                    <InputField
                                        value={settings.webSearch.apiKey}
                                        onChange={v => updateSettings(s => { s.webSearch.apiKey = v; })}
                                        type={showApiKeys['webSearch'] ? 'text' : 'password'}
                                        className="w-52 pr-10"
                                    />
                                    <button
                                        type="button"
                                        onClick={() => setShowApiKeys(p => ({ ...p, webSearch: !p.webSearch }))}
                                        className="absolute right-2 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
                                    >
                                        {showApiKeys['webSearch'] ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                    </button>
                                </div>
                            </SettingRow>
                        </>
                    )}
                </SectionCard>

                <SectionCard title={t('settings.external_services')}>
                    <SettingRow label={t('settings.bilibili_sessdata')} desc={t('settings.bilibili_sessdata_desc')}>
                        <div className="flex items-center gap-3">
                            <span className={`px-2.5 py-1 rounded-md text-fs-xs flex items-center gap-1.5 ${settings.bilibiliSessdata ? 'bg-green-500/10 text-green-500' : 'bg-surface-subtle text-text-tertiary'}`}>
                                <span className={`w-1.5 h-1.5 rounded-full ${settings.bilibiliSessdata ? 'bg-green-500' : 'bg-stroke-divider'}`} />
                                {settings.bilibiliSessdata ? t('settings.status_logged_in') : t('settings.status_not_logged_in')}
                            </span>

                            {settings.bilibiliSessdata ? (
                                <button
                                    onClick={() => {
                                        updateSettings(s => { s.bilibiliSessdata = ''; });
                                        if (settings) {
                                            const copy = JSON.parse(JSON.stringify(settings)) as AllSettings;
                                            copy.bilibiliSessdata = '';
                                            invoke('save_settings', { settings: copy }).catch(console.error);
                                        }
                                    }}
                                    className="text-fs-xs text-red-500 hover:bg-red-500/10 px-3 py-1.5 rounded-lg transition-colors border border-red-500/20"
                                >
                                    {t('settings.logout')}
                                </button>
                            ) : (
                                <button
                                    onClick={handleBilibiliLogin}
                                    disabled={bilibiliLoggingIn}
                                    className="text-fs-xs bg-accent-default text-white hover:bg-accent-light1 px-3 py-1.5 rounded-lg transition-colors disabled:opacity-50"
                                >
                                    {bilibiliLoggingIn ? t('common.loading') : t('settings.login_bilibili')}
                                </button>
                            )}
                        </div>
                    </SettingRow>
                </SectionCard>

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? t('common.saving') : t('common.save')}
                    </button>
                </div>
            </div>
        );
    };

    // ── Tab: 知識庫 ──────────────────────────────────────────

    const renderKnowledge = () => {
        if (!settings) return null;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.knowledge_section_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.knowledge_section_desc')}</p>
                </div>

                <SectionCard title={t('settings.knowledge_setting')}>
                    <SettingRow label={t('settings.knowledge_path')} desc={t('settings.knowledge_path_desc')}>
                        <InputField value={settings.knowledge.kbPath} onChange={v => updateSettings(s => { s.knowledge.kbPath = v; })} placeholder="path/to/kb" className="w-64" />
                    </SettingRow>
                    <SettingRow label={t('settings.auto_classify')} desc={t('settings.auto_classify_desc')}>
                        <Toggle checked={settings.knowledge.autoClassifyEnabled} onChange={v => updateSettings(s => { s.knowledge.autoClassifyEnabled = v; })} />
                    </SettingRow>
                    <SettingRow label={t('settings.auto_space_mode')} desc={t('settings.auto_space_mode_desc')}>
                        <SelectField
                            value={settings.knowledge.autoSpaceMode}
                            onChange={v => updateSettings(s => { s.knowledge.autoSpaceMode = v; })}
                            options={[{ value: 'suggest', label: t('settings.auto_space_suggest') }, { value: 'auto', label: t('settings.auto_space_auto') }]}
                        />
                    </SettingRow>
                </SectionCard>

                <SectionCard title={t('settings.knowledge_management')} desc={t('settings.knowledge_management_desc')}>
                    <div className="grid grid-cols-2 gap-3">
                        <button
                            onClick={async () => {
                                const tid = toast.loading(t('common.loading'));
                                try {
                                    await invoke('rebuild_kb_index');
                                    toast.success(t('common.success'), { id: tid });
                                } catch (e: any) {
                                    toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <RefreshCw className="w-4 h-4 text-accent-default" />
                            {t('settings.rebuild_index')}
                        </button>
                        <button
                            onClick={async () => {
                                try {
                                    const dest = await save({
                                        defaultPath: 'insightcap-kb-export.zip',
                                        filters: [{ name: 'InsightCAP KB', extensions: ['zip'] }],
                                    });
                                    if (!dest) return;
                                    const mnemonic: string = await invoke('generate_recovery_phrase');
                                    setExportModal({ open: true, step: 'mnemonic', mnemonic, confirmed: false, destPath: dest, loading: false });
                                } catch (e) {
                                    console.error('Export dialog failed', e);
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Download className="w-4 h-4 text-accent-default" />
                            {t('settings.export_kb')}
                        </button>
                        <button
                            onClick={async () => {
                                try {
                                    const selected = await open({
                                        multiple: false,
                                        filters: [{ name: 'InsightCAP KB', extensions: ['zip'] }],
                                    });
                                    if (!selected || typeof selected !== 'string') return;
                                    setImportModal({ open: true, step: 'input', srcPath: selected, mnemonic: '', newPassword: '', newMnemonic: '', confirmed: false, loading: false, error: '' });
                                } catch (e) {
                                    console.error('Import dialog failed', e);
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Upload className="w-4 h-4 text-accent-default" />
                            {t('settings.import_kb')}
                        </button>
                        <button
                            onClick={async () => {
                                if (!window.confirm(t('common.warning_irreversible'))) return;
                                const tid = toast.loading(t('common.loading'));
                                try {
                                    await invoke('delete_kb');
                                    toast.success(t('common.success'), { id: tid });
                                } catch (e: any) {
                                    toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-red-500/20 rounded-lg px-4 py-3 text-sm text-red-500 hover:bg-red-500/5 transition-colors"
                        >
                            <AlertTriangle className="w-4 h-4" />
                            {t('settings.delete_kb')}
                        </button>
                        <button
                            onClick={async () => {
                                const tid = toast.loading(t('common.loading'));
                                try {
                                    const msg = await invoke<string>('repair_missing_local_copies');
                                    toast.success(msg, { id: tid });
                                } catch (e: any) {
                                    toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Wrench className="w-4 h-4 text-accent-default" />
                            {t('settings.repair_files')}
                        </button>
                    </div>
                </SectionCard>

                <SectionCard
                    title={t('settings.external_kb')}
                    desc={t('settings.external_kb_desc')}
                    action={
                        <button onClick={handleAddKb} disabled={kbLoading} className="flex items-center gap-1.5 bg-accent-default text-white px-3 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                            <Plus className="w-4 h-4" /> {t('common.add')}
                        </button>
                    }
                >
                    {externalKbs.length === 0 ? (
                        <div className="text-center py-8 text-text-tertiary">
                            <Database className="w-7 h-7 mx-auto mb-2 opacity-50" />
                            <p className="text-fs-sm">{t('settings.external_kb_empty')}</p>
                        </div>
                    ) : (
                        externalKbs.map(kb => (
                            <div key={kb.id} className="flex items-start justify-between gap-3 bg-surface-base rounded-lg px-4 py-3 border border-stroke-divider">
                                <div className="min-w-0">
                                    <div className="flex items-center gap-2 mb-1">
                                        <Database className="w-4 h-4 text-accent-default shrink-0" />
                                        <span className="font-medium text-text-primary text-fs-sm">{kb.name}</span>
                                        <span className={`text-fs-xs px-1.5 py-0.5 rounded-full flex items-center gap-1 ${kb.status === 'connected' ? 'bg-green-500/10 text-green-500' : 'bg-red-500/10 text-red-500'}`}>
                                            <span className={`w-1.5 h-1.5 rounded-full ${kb.status === 'connected' ? 'bg-green-500' : 'bg-red-500'}`} />
                                            {kb.status === 'connected' ? t('settings.kb_connected') : t('settings.kb_error')}
                                        </span>
                                    </div>
                                    <p className="text-fs-xs text-text-tertiary">{kb.dbPath} · {kb.embeddingModel} ({kb.embeddingDimension}{t('common.dimension')})</p>
                                </div>
                                <button onClick={() => handleRemoveKb(kb.id, kb.name)} className="p-1.5 text-text-tertiary hover:text-red-500 rounded-md transition-colors" title={t('common.delete')}>
                                    <Trash2 className="w-4 h-4" />
                                </button>
                            </div>
                        ))
                    )}
                </SectionCard>

                <div className="flex justify-end">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? t('common.saving') : t('common.save')}
                    </button>
                </div>
            </div>
        );
    };

    // ── Tab: 其他 ────────────────────────────────────────────

    const renderOther = () => {
        if (!settings) return null;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.other_section_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.other_section_desc')}</p>
                </div>

                <SectionCard title={t('settings.editor_setting')}>
                    <SettingRow label={t('settings.default_font')}>
                        <InputField value={settings.editor.defaultFont} onChange={v => updateSettings(s => { s.editor.defaultFont = v; })} className="w-52" />
                    </SettingRow>
                    <SettingRow label={t('settings.font_size')}>
                        <InputField value={settings.editor.defaultFontSize} onChange={v => updateSettings(s => { s.editor.defaultFontSize = v; })} className="w-20" />
                    </SettingRow>
                    <SettingRow label={t('settings.line_spacing')}>
                        <InputField value={settings.editor.defaultLineSpacing} onChange={v => updateSettings(s => { s.editor.defaultLineSpacing = v; })} className="w-20" />
                    </SettingRow>
                    <SettingRow label={t('settings.export_format')}>
                        <SelectField
                            value={settings.editor.defaultExportFormat}
                            onChange={v => updateSettings(s => { s.editor.defaultExportFormat = v; })}
                            options={[
                                { value: 'md', label: t('settings.format_markdown') },
                                { value: 'docx', label: t('settings.format_word') },
                                { value: 'txt', label: t('settings.format_text') }
                            ]}
                        />
                    </SettingRow>
                </SectionCard>

                <SectionCard title={t('settings.auto_cleanup')}>
                    <SettingRow label={t('settings.auto_cleanup_enable')} desc={t('settings.auto_cleanup_desc')}>
                        <Toggle checked={settings.autoCleanup.enabled} onChange={v => updateSettings(s => { s.autoCleanup.enabled = v; })} />
                    </SettingRow>
                    {settings.autoCleanup.enabled && (
                        <SettingRow label={t('settings.retention_days')}>
                            <InputField
                                value={String(settings.autoCleanup.retentionDays)}
                                onChange={v => updateSettings(s => { s.autoCleanup.retentionDays = parseInt(v) || 0; })}
                                type="number"
                                className="w-20"
                            />
                        </SettingRow>
                    )}
                </SectionCard>

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? t('common.saving') : t('common.save')}
                    </button>
                </div>
            </div>
        );
    };

    // ── Render ────────────────────────────────────────────

    return (
        <div className="flex h-full w-full bg-surface-base">
            {/* Sidebar */}
            <div className="w-64 border-r border-stroke-divider bg-surface-layer/30 p-6 flex flex-col gap-1.5 overflow-y-auto">
                <div className="px-3 mb-6">
                    <h2 className="text-fs-xl font-bold text-text-primary">{t('settings.nav_title')}</h2>
                    <p className="text-fs-xs text-text-tertiary">{t('settings.nav_subtitle')}</p>
                </div>
                {TABS.map(tab => {
                    const Icon = tab.icon;
                    const isActive = activeTab === tab.id;
                    return (
                        <button
                            key={tab.id}
                            onClick={() => setActiveTab(tab.id)}
                            className={`flex items-center gap-3 text-left px-3.5 py-2.5 rounded-xl transition-all duration-200 text-fs-sm group ${isActive
                                ? 'bg-accent-default text-white shadow-lg shadow-accent-default/20 font-medium active:scale-95'
                                : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'
                                }`}
                        >
                            <Icon className={`w-4.5 h-4.5 transition-colors ${isActive ? 'text-white' : 'text-text-tertiary group-hover:text-text-primary'}`} />
                            {t(tab.labelKey as any)}
                        </button>
                    );
                })}
            </div>

            {/* Content */}
            <div className="flex-1 p-10 overflow-y-auto bg-surface-base/50">
                {activeTab === 'general' && renderGeneral()}
                {activeTab === 'provider' && renderProvider()}
                {activeTab === 'ai' && renderAI()}
                {activeTab === 'knowledge' && renderKnowledge()}
                {activeTab === 'other' && renderOther()}
            </div>

            {/* ── 匯出備份恢復碼 Modal ── */}
            {exportModal.open && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
                    <div className="bg-surface-layer border border-stroke-divider rounded-xl p-6 w-[480px] max-w-full shadow-xl">
                        <h3 className="text-fs-xl font-semibold text-text-primary mb-2">{t('settings.export_title')}</h3>
                        <p className="text-fs-sm text-text-secondary mb-4" dangerouslySetInnerHTML={{ __html: t('settings.export_desc') }} />
                        <div className="bg-surface-base border border-stroke-divider rounded-lg p-4 mb-4 font-mono text-fs-sm text-text-primary leading-relaxed break-words">
                            {exportModal.mnemonic}
                        </div>
                        <button
                            onClick={() => {
                                navigator.clipboard.writeText(exportModal.mnemonic);
                                toast.success(t('common.copied'));
                            }}
                            className="text-fs-xs text-accent-default hover:underline mb-4 block"
                        >
                            {t('settings.copy_mnemonic')}
                        </button>
                        <label className="flex items-start gap-2 text-fs-sm text-text-secondary cursor-pointer mb-5">
                            <input
                                type="checkbox"
                                checked={exportModal.confirmed}
                                onChange={e => setExportModal(s => ({ ...s, confirmed: e.target.checked }))}
                                className="mt-0.5 shrink-0"
                            />
                            {t('settings.save_mnemonic_confirm')}
                        </label>
                        <div className="flex gap-3 justify-end">
                            <button
                                onClick={() => setExportModal(s => ({ ...s, open: false }))}
                                className="px-4 py-2 text-fs-sm text-text-secondary hover:text-text-primary border border-stroke-divider rounded-lg transition-colors"
                            >
                                {t('common.cancel')}
                            </button>
                            <button
                                disabled={!exportModal.confirmed || exportModal.loading}
                                onClick={async () => {
                                    setExportModal(s => ({ ...s, loading: true }));
                                    const tid = toast.loading(t('settings.exporting'));
                                    try {
                                        await invoke('export_kb', { destPath: exportModal.destPath, mnemonic: exportModal.mnemonic });
                                        toast.success(t('settings.export_success'), { id: tid });
                                        setExportModal(s => ({ ...s, open: false }));
                                    } catch (e: any) {
                                        toast.error(`${t('settings.export_failed')}: ${e.toString()}`, { id: tid });
                                        setExportModal(s => ({ ...s, loading: false }));
                                    }
                                }}
                                className="px-4 py-2 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors disabled:opacity-50"
                            >
                                {exportModal.loading ? t('settings.exporting') : t('settings.confirm_export')}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* ── 匯入知識庫 Modal ── */}
            {importModal.open && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
                    <div className="bg-surface-layer border border-stroke-divider rounded-xl p-6 w-[480px] max-w-full shadow-xl">
                        {importModal.step === 'input' ? (
                            <>
                                <h3 className="text-fs-xl font-semibold text-text-primary mb-2">{t('settings.import_title')}</h3>
                                <p className="text-fs-sm text-text-secondary mb-4">
                                    {t('settings.import_desc')}
                                </p>
                                <div className="space-y-3 mb-4">
                                    <div>
                                        <label className="block text-fs-sm font-medium text-text-primary mb-1">{t('settings.mnemonic_label')}</label>
                                        <textarea
                                            value={importModal.mnemonic}
                                            onChange={e => setImportModal(s => ({ ...s, mnemonic: e.target.value, error: '' }))}
                                            rows={3}
                                            placeholder={t('settings.import_mnemonic_placeholder')}
                                            className="w-full rounded-md border border-stroke-divider px-3 py-2 text-fs-sm font-mono bg-surface-base text-text-primary focus:outline-none focus:ring-1 focus:ring-stroke-focus resize-none"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-fs-sm font-medium text-text-primary mb-1">{t('settings.new_password_label')}</label>
                                        <input
                                            type="password"
                                            value={importModal.newPassword}
                                            onChange={e => setImportModal(s => ({ ...s, newPassword: e.target.value, error: '' }))}
                                            placeholder={t('settings.password_placeholder')}
                                            className="w-full rounded-md border border-stroke-divider px-3 py-2 text-fs-sm bg-surface-base text-text-primary focus:outline-none focus:ring-1 focus:ring-stroke-focus"
                                        />
                                    </div>
                                </div>
                                {importModal.error && (
                                    <p className="text-fs-sm text-red-500 mb-3">{importModal.error}</p>
                                )}
                                <div className="flex gap-3 justify-end">
                                    <button
                                        onClick={() => setImportModal(s => ({ ...s, open: false }))}
                                        className="px-4 py-2 text-fs-sm text-text-secondary hover:text-text-primary border border-stroke-divider rounded-lg transition-colors"
                                    >
                                        {t('common.cancel')}
                                    </button>
                                    <button
                                        disabled={importModal.loading || importModal.mnemonic.trim().split(/\s+/).length < 24 || importModal.newPassword.length < 8}
                                        onClick={async () => {
                                            setImportModal(s => ({ ...s, loading: true, error: '' }));
                                            try {
                                                const newMnemonic: string = await invoke('import_kb', {
                                                    srcPath: importModal.srcPath,
                                                    mnemonic: importModal.mnemonic.trim(),
                                                    newPassword: importModal.newPassword,
                                                });
                                                setImportModal(s => ({ ...s, loading: false, step: 'new_recovery', newMnemonic, confirmed: false }));
                                            } catch (e: any) {
                                                const msg = e.toString().includes('INVALID_MNEMONIC') ? t('settings.invalid_mnemonic') : `${t('common.error')}: ${e.toString()}`;
                                                setImportModal(s => ({ ...s, loading: false, error: msg }));
                                            }
                                        }}
                                        className="px-4 py-2 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors disabled:opacity-50"
                                    >
                                        {importModal.loading ? t('settings.restoring') : t('settings.restore_kb')}
                                    </button>
                                </div>
                            </>
                        ) : (
                            <>
                                <h3 className="text-fs-xl font-semibold text-text-primary mb-2">{t('settings.import_success_title')}</h3>
                                <p className="text-fs-sm text-text-secondary mb-4">
                                    {t('settings.import_success_desc')}
                                </p>
                                <div className="bg-surface-base border border-stroke-divider rounded-lg p-4 mb-4 font-mono text-fs-sm text-text-primary leading-relaxed break-words">
                                    {importModal.newMnemonic}
                                </div>
                                <button
                                    onClick={() => {
                                        navigator.clipboard.writeText(importModal.newMnemonic);
                                        toast.success(t('common.copied'));
                                    }}
                                    className="text-fs-xs text-accent-default hover:underline mb-4 block"
                                >
                                    {t('settings.copy_mnemonic')}
                                </button>
                                <label className="flex items-start gap-2 text-fs-sm text-text-secondary cursor-pointer mb-5">
                                    <input
                                        type="checkbox"
                                        checked={importModal.confirmed}
                                        onChange={e => setImportModal(s => ({ ...s, confirmed: e.target.checked }))}
                                        className="mt-0.5 shrink-0"
                                    />
                                    {t('settings.save_new_mnemonic_confirm')}
                                </label>
                                <div className="flex justify-end">
                                    <button
                                        disabled={!importModal.confirmed}
                                        onClick={async () => {
                                            await invoke('restart_app');
                                        }}
                                        className="px-4 py-2 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors disabled:opacity-50"
                                    >
                                        {t('settings.restart_app')}
                                    </button>
                                </div>
                            </>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
};
