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

const TABS: { id: SettingsTab; label: string; icon: React.FC<{ className?: string }> }[] = [
    { id: 'general', label: '一般設定', icon: Settings2 },
    { id: 'provider', label: 'LLM Provider', icon: Server },
    { id: 'ai', label: 'AI 設置', icon: Sparkles },
    { id: 'knowledge', label: '知識庫', icon: BookOpen },
    { id: 'other', label: '其他', icon: Ellipsis },
];

const THEMES: { value: Theme; label: string; desc: string }[] = [
    { value: 'frost', label: 'Frost Glass', desc: '亮色・冰藍' },
    { value: 'void', label: 'Deep Void', desc: '暗色・靛紫' },
    { value: 'warm', label: 'Warm Parchment', desc: '亮色・琥珀' },
    { value: 'sage', label: 'Sage Breeze', desc: '亮色・草地綠' },
];

const LANGUAGES: { value: Language; label: string }[] = [
    { value: 'zh-TW', label: '繁體中文' },
    { value: 'zh-CN', label: '简体中文' },
    { value: 'en', label: 'English' },
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
    <div className="bg-surface-layer border border-stroke-divider rounded-xl p-6 mb-4">
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
    const [activeTab, setActiveTab] = useState<SettingsTab>('general');
    const [settings, setSettings] = useState<AllSettings | null>(null);
    const [saving, setSaving] = useState(false);
    const [externalKbs, setExternalKbs] = useState<ExternalKnowledgeBase[]>([]);
    const [kbLoading, setKbLoading] = useState(false);

    // Provider edit state
    const [editingProfile, setEditingProfile] = useState<ProviderProfileData | null>(null);
    const [showApiKeys, setShowApiKeys] = useState<Record<string, boolean>>({});

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
        const tid = toast.loading('正在測試連線...');
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
            toast.success('連線成功！', { id: tid });
        } catch (e: any) {
            toast.error(`連線失敗: ${e.toString()}`, { id: tid });
        }
    };

    // ── Tab: 一般設定 ────────────────────────────────────

    const renderGeneral = () => {
        if (!settings) return null;
        return (
            <div className="max-w-2xl">
                <h3 className="text-fs-2xl font-bold text-text-primary mb-6">一般設定</h3>

                <SectionCard title="語言與外觀">
                    <SettingRow label="介面語言" desc="變更後立即生效">
                        <SelectField
                            value={language}
                            onChange={v => setLanguage(v as Language)}
                            options={LANGUAGES}
                        />
                    </SettingRow>
                    <SettingRow label="主題風格">
                        <div className="flex gap-2">
                            {THEMES.map(t => (
                                <button
                                    key={t.value}
                                    onClick={() => setTheme(t.value)}
                                    className={`px-3 py-1.5 rounded-lg text-fs-xs transition-colors border ${theme === t.value
                                        ? 'border-accent-default bg-accent-default/10 text-accent-default font-medium'
                                        : 'border-stroke-divider text-text-secondary hover:bg-surface-subtle'
                                        }`}
                                    title={t.desc}
                                >
                                    {t.label}
                                </button>
                            ))}
                        </div>
                    </SettingRow>
                </SectionCard>

                <SectionCard title="系統行為">
                    <SettingRow label="開機時自動啟動" desc="系統啟動時自動開啟 InsightCAP">
                        <Toggle
                            checked={settings?.general.launchAtStartup ?? false}
                            onChange={v => { updateSettings(s => { s.general.launchAtStartup = v; }); }}
                        />
                    </SettingRow>
                    <SettingRow label="最小化至系統匣" desc="關閉視窗時最小化至系統匣">
                        <Toggle
                            checked={settings?.general.minimizeToTray ?? true}
                            onChange={v => { updateSettings(s => { s.general.minimizeToTray = v; }); }}
                        />
                    </SettingRow>
                </SectionCard>

                <SectionCard title="快捷鍵">
                    <SettingRow label="擷取剪貼簿" desc="全域快捷鍵">
                        <InputField value={settings.hotkeys.captureClipboard} onChange={v => updateSettings(s => { s.hotkeys.captureClipboard = v; })} className="w-40" />
                    </SettingRow>
                    <SettingRow label="快速輸入" desc="全域快捷鍵">
                        <InputField value={settings.hotkeys.quickInput} onChange={v => updateSettings(s => { s.hotkeys.quickInput = v; })} className="w-40" />
                    </SettingRow>
                </SectionCard>

                <SectionCard title="外部服務授權">
                    <SettingRow label="Bilibili SESSDATA" desc="需登入以提取 B 站影片字幕（可於 B 站登入後檢視 Cookie 取得）">
                        <div className="relative">
                            <InputField
                                value={settings.bilibiliSessdata ?? ''}
                                onChange={v => updateSettings(s => { s.bilibiliSessdata = v; })}
                                type={showApiKeys['bilibili'] ? 'text' : 'password'}
                                placeholder="請輸入 SESSDATA..."
                                className="w-52 pr-10"
                            />
                            <button
                                type="button"
                                onClick={() => setShowApiKeys(p => ({ ...p, bilibili: !p.bilibili }))}
                                className="absolute right-2 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
                            >
                                {showApiKeys['bilibili'] ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                            </button>
                        </div>
                    </SettingRow>
                </SectionCard>

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? '儲存中...' : '儲存'}
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
            <div className="max-w-3xl">
                <h3 className="text-fs-2xl font-bold text-text-primary mb-6">LLM Provider</h3>

                <SectionCard
                    title="Provider 管理"
                    desc="新增與管理 LLM 服務商的連線設定，在 AI 設置中可選擇使用哪個 Provider。"
                    action={
                        <button
                            onClick={() => setEditingProfile({ id: crypto.randomUUID(), name: '', provider: 'ollama', baseUrl: 'http://localhost:11434', apiKey: '' })}
                            className="flex items-center gap-1.5 bg-accent-default text-white px-3 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors"
                        >
                            <Plus className="w-4 h-4" /> 新增
                        </button>
                    }
                >
                    {(settings?.aiModels.providerProfiles ?? []).length === 0 && !editingProfile && (
                        <div className="text-center py-8 text-text-tertiary">
                            <Server className="w-7 h-7 mx-auto mb-2 opacity-50" />
                            <p className="text-fs-sm">尚未新增任何 Provider</p>
                        </div>
                    )}

                    {(settings?.aiModels.providerProfiles ?? []).map(p => (
                        <div key={p.id} className="flex items-center justify-between gap-3 bg-surface-base rounded-lg px-4 py-3 border border-stroke-divider">
                            <div className="min-w-0">
                                <div className="font-medium text-text-primary text-fs-sm">{p.name || '(未命名)'}</div>
                                <div className="text-fs-xs text-text-tertiary mt-0.5">
                                    {PROVIDER_OPTIONS.find(o => o.value === p.provider)?.label ?? p.provider}
                                    {p.baseUrl ? ` · ${p.baseUrl}` : ''}
                                </div>
                            </div>
                            <div className="flex items-center gap-1">
                                <button onClick={() => handleTestConnection(p)} className="p-1.5 text-text-tertiary hover:text-accent-default rounded-md transition-colors" title="測試連線">
                                    <Zap className="w-4 h-4" />
                                </button>
                                <button onClick={() => setEditingProfile({ ...p })} className="p-1.5 text-text-tertiary hover:text-accent-default rounded-md transition-colors" title="編輯">
                                    <Settings2 className="w-4 h-4" />
                                </button>
                                <button onClick={() => handleDeleteProfile(p.id)} className="p-1.5 text-text-tertiary hover:text-red-500 rounded-md transition-colors" title="刪除">
                                    <Trash2 className="w-4 h-4" />
                                </button>
                            </div>
                        </div>
                    ))}

                    {editingProfile && (
                        <div className="bg-surface-base rounded-lg p-4 border border-accent-default/30 space-y-3">
                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">名稱</label>
                                    <InputField value={editingProfile.name} onChange={v => setEditingProfile({ ...editingProfile, name: v })} placeholder="例: My OpenAI" className="w-full" />
                                </div>
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">Provider 類型</label>
                                    <SelectField value={editingProfile.provider} onChange={v => setEditingProfile({ ...editingProfile, provider: v })} options={PROVIDER_OPTIONS} className="w-full" />
                                </div>
                            </div>
                            <div>
                                <label className="text-fs-xs text-text-secondary mb-1 block">Base URL</label>
                                <InputField value={editingProfile.baseUrl ?? ''} onChange={v => setEditingProfile({ ...editingProfile, baseUrl: v })} placeholder="http://localhost:11434" className="w-full" />
                            </div>
                            {editingProfile.provider !== 'ollama' && (
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">API Key</label>
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
                                <button onClick={() => setEditingProfile(null)} className="px-3 py-1.5 rounded-lg text-fs-sm text-text-secondary hover:bg-surface-subtle transition-colors">取消</button>
                                <button onClick={() => handleTestConnection(editingProfile)} className="px-3 py-1.5 rounded-lg text-fs-sm text-text-secondary hover:bg-surface-subtle transition-colors">測試</button>
                                <button onClick={handleSaveProfile} className="bg-accent-default text-white px-4 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors">儲存</button>
                            </div>
                        </div>
                    )}
                </SectionCard>

                <SectionCard title="模型配置" desc="為不同功能指定使用的 LLM 模型。">
                    {renderModelField('聊天模型', '用於對話、問答功能', ai.chatLlm, m => updateSettings(s => { s.aiModels.chatLlm = m; }))}
                    {renderModelField('內容處理模型', '用於自動分類、標籤提取等背景處理', ai.contentProcessorLlm, m => updateSettings(s => { s.aiModels.contentProcessorLlm = m; }))}
                    {renderModelField('視覺模型', '用於圖片 OCR 與視覺理解', ai.visionModel, m => updateSettings(s => { s.aiModels.visionModel = m; }))}
                    {renderModelField('Embedding 模型', '用於語意向量索引（RAG 檢索）', ai.embeddingModel, m => updateSettings(s => { s.aiModels.embeddingModel = m; }))}
                </SectionCard>

                <SectionCard title="摘要模型">
                    <SettingRow label="摘要模型" desc="用於對話摘要，'follow_chat' 表示跟隨聊天模型">
                        <InputField
                            value={ai.summaryModel ?? 'follow_chat'}
                            onChange={v => updateSettings(s => { s.aiModels.summaryModel = v; })}
                            className="w-48"
                        />
                    </SettingRow>
                </SectionCard>

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? '儲存中...' : '儲存'}
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
                    <label className="text-fs-xs text-text-secondary mb-1 block">Provider</label>
                    <SelectField value={model.provider} onChange={v => onChange({ ...model, provider: v })} options={[...PROVIDER_OPTIONS, { value: 'local', label: 'Local' }]} className="w-full" />
                </div>
                <div>
                    <label className="text-fs-xs text-text-secondary mb-1 block">模型名稱</label>
                    <InputField value={model.model} onChange={v => onChange({ ...model, model: v })} placeholder="模型名稱" className="w-full" />
                </div>
            </div>
        </div>
    );

    const renderAI = () => {
        if (!settings) return null;
        return (
            <div className="max-w-2xl">
                <h3 className="text-fs-2xl font-bold text-text-primary mb-6">AI 設置</h3>

                <SectionCard title="AI 回答偏好" desc="附加在每次 AI 回答的風格指令，例如「請用英文回答，語氣要簡潔」。系統指引優先，此設定不可覆蓋系統行為。">
                    <textarea
                        value={settings.chatPromptInstruction}
                        onChange={e => updateSettings(s => { s.chatPromptInstruction = e.target.value; })}
                        placeholder="留空表示使用預設行為..."
                        rows={4}
                        className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-none focus:outline-none focus:ring-1 focus:ring-accent-default"
                    />
                </SectionCard>

                <SectionCard title="網路搜尋">
                    <SettingRow label="啟用網路搜尋">
                        <Toggle checked={settings.webSearch.enabled} onChange={v => updateSettings(s => { s.webSearch.enabled = v; })} />
                    </SettingRow>
                    {settings.webSearch.enabled && (
                        <>
                            <SettingRow label="搜尋提供者">
                                <SelectField
                                    value={settings.webSearch.provider}
                                    onChange={v => updateSettings(s => { s.webSearch.provider = v; })}
                                    options={[{ value: 'tavily', label: 'Tavily' }, { value: 'serper', label: 'Serper' }]}
                                />
                            </SettingRow>
                            <SettingRow label="API Key">
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

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? '儲存中...' : '儲存'}
                    </button>
                </div>
            </div>
        );
    };

    // ── Tab: 知識庫 ──────────────────────────────────────────

    const renderKnowledge = () => {
        if (!settings) return null;
        return (
            <div className="max-w-3xl">
                <h3 className="text-fs-2xl font-bold text-text-primary mb-6">知識庫</h3>

                <SectionCard title="知識庫設定">
                    <SettingRow label="知識庫路徑" desc="本地知識庫的儲存位置">
                        <InputField value={settings.knowledge.kbPath} onChange={v => updateSettings(s => { s.knowledge.kbPath = v; })} placeholder="path/to/kb" className="w-64" />
                    </SettingRow>
                    <SettingRow label="自動分類" desc="擷取新內容時自動分類到對應空間">
                        <Toggle checked={settings.knowledge.autoClassifyEnabled} onChange={v => updateSettings(s => { s.knowledge.autoClassifyEnabled = v; })} />
                    </SettingRow>
                    <SettingRow label="自動空間模式" desc="suggest = 建議確認，auto = 直接建立">
                        <SelectField
                            value={settings.knowledge.autoSpaceMode}
                            onChange={v => updateSettings(s => { s.knowledge.autoSpaceMode = v; })}
                            options={[{ value: 'suggest', label: '建議確認' }, { value: 'auto', label: '自動建立' }]}
                        />
                    </SettingRow>
                </SectionCard>

                <SectionCard title="知識庫管理" desc="對本地知識庫執行維護操作。">
                    <div className="grid grid-cols-2 gap-3">
                        <button
                            onClick={async () => {
                                const tid = toast.loading('正在重建索引...');
                                try {
                                    await invoke('rebuild_kb_index');
                                    toast.success('索引重建完成', { id: tid });
                                } catch (e: any) {
                                    toast.error(`重建失敗: ${e.toString()}`, { id: tid });
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <RefreshCw className="w-4 h-4 text-accent-default" />
                            重建知識庫索引
                        </button>
                        <button
                            onClick={async () => {
                                try {
                                    const dest = await save({
                                        defaultPath: 'insightcap-kb-export.zip',
                                        filters: [{ name: 'InsightCAP 知識庫封包', extensions: ['zip'] }],
                                    });
                                    if (!dest) return;
                                    // 生成備份恢復碼後開 modal
                                    const mnemonic: string = await invoke('generate_recovery_phrase');
                                    setExportModal({ open: true, step: 'mnemonic', mnemonic, confirmed: false, destPath: dest, loading: false });
                                } catch (e) {
                                    console.error('Export dialog failed', e);
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Download className="w-4 h-4 text-accent-default" />
                            匯出知識庫
                        </button>
                        <button
                            onClick={async () => {
                                try {
                                    const selected = await open({
                                        multiple: false,
                                        filters: [{ name: 'InsightCAP 知識庫封包', extensions: ['zip'] }],
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
                            匯入知識庫
                        </button>
                        <button
                            onClick={async () => {
                                if (!window.confirm('⚠️ 確定要刪除整個知識庫嗎？此操作無法復原！')) return;
                                if (!window.confirm('再次確認：所有知識庫資料將被永久刪除。')) return;
                                const tid = toast.loading('正在刪除知識庫...');
                                try {
                                    await invoke('delete_kb');
                                    toast.success('知識庫已刪除', { id: tid });
                                } catch (e: any) {
                                    toast.error(`刪除失敗: ${e.toString()}`, { id: tid });
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-red-500/20 rounded-lg px-4 py-3 text-sm text-red-500 hover:bg-red-500/5 transition-colors"
                        >
                            <AlertTriangle className="w-4 h-4" />
                            刪除知識庫
                        </button>
                        <button
                            onClick={async () => {
                                const tid = toast.loading('正在掃描並修復歷史資料...');
                                try {
                                    const msg = await invoke<string>('repair_missing_local_copies');
                                    toast.success(msg, { id: tid });
                                } catch (e: any) {
                                    toast.error(`修復失敗: ${e.toString()}`, { id: tid });
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Wrench className="w-4 h-4 text-accent-default" />
                            修復歷史檔案副本
                        </button>
                    </div>
                </SectionCard>

                <SectionCard
                    title="外部知識庫"
                    desc="掛載標準化的外部知識庫檔案（.db / .sqlite），啟動企業級混合 RAG 檢索。"
                    action={
                        <button onClick={handleAddKb} disabled={kbLoading} className="flex items-center gap-1.5 bg-accent-default text-white px-3 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                            <Plus className="w-4 h-4" /> 載入
                        </button>
                    }
                >
                    {externalKbs.length === 0 ? (
                        <div className="text-center py-8 text-text-tertiary">
                            <Database className="w-7 h-7 mx-auto mb-2 opacity-50" />
                            <p className="text-fs-sm">尚未掛載任何外部知識庫</p>
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
                                            {kb.status === 'connected' ? '已連線' : '異常'}
                                        </span>
                                    </div>
                                    <p className="text-fs-xs text-text-tertiary">{kb.dbPath} · {kb.embeddingModel} ({kb.embeddingDimension}維)</p>
                                </div>
                                <button onClick={() => handleRemoveKb(kb.id, kb.name)} className="p-1.5 text-text-tertiary hover:text-red-500 rounded-md transition-colors" title="移除連線">
                                    <Trash2 className="w-4 h-4" />
                                </button>
                            </div>
                        ))
                    )}
                </SectionCard>

                <div className="flex justify-end">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? '儲存中...' : '儲存'}
                    </button>
                </div>
            </div>
        );
    };

    // ── Tab: 其他 ────────────────────────────────────────────

    const renderOther = () => {
        if (!settings) return null;
        return (
            <div className="max-w-2xl">
                <h3 className="text-fs-2xl font-bold text-text-primary mb-6">其他</h3>

                <SectionCard title="編輯器設定">
                    <SettingRow label="預設字型">
                        <InputField value={settings.editor.defaultFont} onChange={v => updateSettings(s => { s.editor.defaultFont = v; })} className="w-52" />
                    </SettingRow>
                    <SettingRow label="字型大小">
                        <InputField value={settings.editor.defaultFontSize} onChange={v => updateSettings(s => { s.editor.defaultFontSize = v; })} className="w-20" />
                    </SettingRow>
                    <SettingRow label="行距">
                        <InputField value={settings.editor.defaultLineSpacing} onChange={v => updateSettings(s => { s.editor.defaultLineSpacing = v; })} className="w-20" />
                    </SettingRow>
                    <SettingRow label="預設匯出格式">
                        <SelectField
                            value={settings.editor.defaultExportFormat}
                            onChange={v => updateSettings(s => { s.editor.defaultExportFormat = v; })}
                            options={[{ value: 'md', label: 'Markdown' }, { value: 'docx', label: 'Word (.docx)' }, { value: 'txt', label: '純文字' }]}
                        />
                    </SettingRow>
                </SectionCard>

                <SectionCard title="自動清理">
                    <SettingRow label="啟用自動清理" desc="自動清除過期的暫存資料">
                        <Toggle checked={settings.autoCleanup.enabled} onChange={v => updateSettings(s => { s.autoCleanup.enabled = v; })} />
                    </SettingRow>
                    {settings.autoCleanup.enabled && (
                        <SettingRow label="保留天數">
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
                        {saving ? '儲存中...' : '儲存'}
                    </button>
                </div>
            </div>
        );
    };

    // ── Render ────────────────────────────────────────────

    return (
        <div className="flex h-full w-full bg-surface-base">
            {/* Sidebar */}
            <div className="w-56 border-r border-stroke-divider p-6 flex flex-col gap-1">
                <h2 className="text-fs-xl font-bold mb-4 text-text-primary">設定</h2>
                {TABS.map(tab => {
                    const Icon = tab.icon;
                    return (
                        <button
                            key={tab.id}
                            onClick={() => setActiveTab(tab.id)}
                            className={`flex items-center gap-2.5 text-left px-3 py-2 rounded-lg transition-colors text-fs-sm ${activeTab === tab.id
                                ? 'bg-surface-layer text-accent-default font-medium'
                                : 'text-text-secondary hover:bg-surface-subtle'
                                }`}
                        >
                            <Icon className="w-4 h-4" />
                            {tab.label}
                        </button>
                    );
                })}
            </div>

            {/* Content */}
            <div className="flex-1 p-8 overflow-y-auto">
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
                        <h3 className="text-fs-xl font-semibold text-text-primary mb-2">匯出知識庫</h3>
                        <p className="text-fs-sm text-text-secondary mb-4">
                            這是本次備份的專屬恢復碼。還原備份時需要輸入此恢復碼，<strong className="text-text-primary">請妥善保存</strong>，遺失後無法還原備份。
                        </p>
                        <div className="bg-surface-base border border-stroke-divider rounded-lg p-4 mb-4 font-mono text-fs-sm text-text-primary leading-relaxed break-words">
                            {exportModal.mnemonic}
                        </div>
                        <button
                            onClick={() => {
                                navigator.clipboard.writeText(exportModal.mnemonic);
                                toast.success('已複製到剪貼簿');
                            }}
                            className="text-fs-xs text-accent-default hover:underline mb-4 block"
                        >
                            複製恢復碼
                        </button>
                        <label className="flex items-start gap-2 text-fs-sm text-text-secondary cursor-pointer mb-5">
                            <input
                                type="checkbox"
                                checked={exportModal.confirmed}
                                onChange={e => setExportModal(s => ({ ...s, confirmed: e.target.checked }))}
                                className="mt-0.5 shrink-0"
                            />
                            我已安全保存備份恢復碼，了解遺失後無法還原此備份
                        </label>
                        <div className="flex gap-3 justify-end">
                            <button
                                onClick={() => setExportModal(s => ({ ...s, open: false }))}
                                className="px-4 py-2 text-fs-sm text-text-secondary hover:text-text-primary border border-stroke-divider rounded-lg transition-colors"
                            >
                                取消
                            </button>
                            <button
                                disabled={!exportModal.confirmed || exportModal.loading}
                                onClick={async () => {
                                    setExportModal(s => ({ ...s, loading: true }));
                                    const tid = toast.loading('正在匯出知識庫...');
                                    try {
                                        await invoke('export_kb', { destPath: exportModal.destPath, mnemonic: exportModal.mnemonic });
                                        toast.success('知識庫已匯出', { id: tid });
                                        setExportModal(s => ({ ...s, open: false }));
                                    } catch (e: any) {
                                        toast.error(`匯出失敗: ${e.toString()}`, { id: tid });
                                        setExportModal(s => ({ ...s, loading: false }));
                                    }
                                }}
                                className="px-4 py-2 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors disabled:opacity-50"
                            >
                                {exportModal.loading ? '匯出中...' : '確認匯出'}
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
                                <h3 className="text-fs-xl font-semibold text-text-primary mb-2">匯入知識庫</h3>
                                <p className="text-fs-sm text-text-secondary mb-4">
                                    請輸入備份時的恢復碼，並設定新密碼。匯入後將重啟應用。
                                </p>
                                <div className="space-y-3 mb-4">
                                    <div>
                                        <label className="block text-fs-sm font-medium text-text-primary mb-1">備份恢復碼（24 個單詞）</label>
                                        <textarea
                                            value={importModal.mnemonic}
                                            onChange={e => setImportModal(s => ({ ...s, mnemonic: e.target.value, error: '' }))}
                                            rows={3}
                                            placeholder="apple bridge cloud dance echo forest..."
                                            className="w-full rounded-md border border-stroke-divider px-3 py-2 text-fs-sm font-mono bg-surface-base text-text-primary focus:outline-none focus:ring-1 focus:ring-stroke-focus resize-none"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-fs-sm font-medium text-text-primary mb-1">設定新密碼</label>
                                        <input
                                            type="password"
                                            value={importModal.newPassword}
                                            onChange={e => setImportModal(s => ({ ...s, newPassword: e.target.value, error: '' }))}
                                            placeholder="至少 8 個字符"
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
                                        取消
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
                                                const msg = e.toString().includes('INVALID_MNEMONIC') ? '恢復碼不正確，請確認輸入無誤' : `匯入失敗: ${e.toString()}`;
                                                setImportModal(s => ({ ...s, loading: false, error: msg }));
                                            }
                                        }}
                                        className="px-4 py-2 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors disabled:opacity-50"
                                    >
                                        {importModal.loading ? '還原中...' : '還原知識庫'}
                                    </button>
                                </div>
                            </>
                        ) : (
                            <>
                                <h3 className="text-fs-xl font-semibold text-text-primary mb-2">⚠️ 請保存新的恢復碼</h3>
                                <p className="text-fs-sm text-text-secondary mb-4">
                                    知識庫已還原成功。以下是新的日常恢復碼，請妥善保存後再重啟應用。
                                </p>
                                <div className="bg-surface-base border border-stroke-divider rounded-lg p-4 mb-4 font-mono text-fs-sm text-text-primary leading-relaxed break-words">
                                    {importModal.newMnemonic}
                                </div>
                                <button
                                    onClick={() => {
                                        navigator.clipboard.writeText(importModal.newMnemonic);
                                        toast.success('已複製到剪貼簿');
                                    }}
                                    className="text-fs-xs text-accent-default hover:underline mb-4 block"
                                >
                                    複製恢復碼
                                </button>
                                <label className="flex items-start gap-2 text-fs-sm text-text-secondary cursor-pointer mb-5">
                                    <input
                                        type="checkbox"
                                        checked={importModal.confirmed}
                                        onChange={e => setImportModal(s => ({ ...s, confirmed: e.target.checked }))}
                                        className="mt-0.5 shrink-0"
                                    />
                                    我已安全保存新的恢復碼
                                </label>
                                <div className="flex justify-end">
                                    <button
                                        disabled={!importModal.confirmed}
                                        onClick={async () => {
                                            await invoke('restart_app');
                                        }}
                                        className="px-4 py-2 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-light1 transition-colors disabled:opacity-50"
                                    >
                                        重啟應用
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
