import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
// import { ExternalKnowledgeBase, ExternalKbLoadResult } from '../lib/types';
import { useThemeStore, type Theme } from '../stores/themeStore';
import { useLanguageStore } from '../stores/languageStore';
import type { Language } from '../i18n';
import {
    Settings2, Server, Sparkles, BookOpen, PenLine,
    Plus, Trash2, Eye, EyeOff, ExternalLink,
    RefreshCw, Download, Upload, AlertTriangle, Wrench,
    User, ShieldCheck, KeyRound,
} from 'lucide-react';
import { useT } from '../hooks/useT';
import { save } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
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
        promptInstructionOverride?: string;
    };
    telegram: {
        botToken: string;
        allowedUserIds: number[];
        enabled: boolean;
        promptInstructionOverride?: string;
    };
    reminders: {
        enabled: boolean;
        dailyReminderTime: string;
        quietHoursStart: string;
        quietHoursEnd: string;
        weekendQuiet: boolean;
    };
    bilibiliSessdata?: string;
    chatPromptInstruction: string;
}

type SettingsTab = 'general' | 'personal' | 'provider' | 'ai' | 'knowledge' | 'other';

// These will be translated in the component
const TABS: { id: SettingsTab; labelKey: string; icon: React.FC<{ className?: string }> }[] = [
    { id: 'general', labelKey: 'settings.general', icon: Settings2 },
    { id: 'personal', labelKey: 'settings.personal', icon: User },
    { id: 'provider', labelKey: 'settings.ai_models', icon: Sparkles },
    { id: 'ai', labelKey: 'settings.ai_section', icon: Server },
    { id: 'knowledge', labelKey: 'settings.knowledge', icon: BookOpen },
    { id: 'other', labelKey: 'settings.other', icon: PenLine },
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

interface ProviderCard {
    value: string;
    label: string;
    desc: string;
    defaultBaseUrl?: string;
    apiUrl?: string;
    emoji: string;
    local?: boolean;
}

const PROVIDER_CARDS: ProviderCard[] = [
    {
        value: 'openai',
        label: 'OpenAI',
        desc: 'GPT-4o, o1, o3...',
        apiUrl: 'https://platform.openai.com/api-keys',
        emoji: '🟢',
    },
    {
        value: 'anthropic',
        label: 'Anthropic',
        desc: 'Claude 4, Claude 3.5...',
        apiUrl: 'https://console.anthropic.com/settings/keys',
        emoji: '🟠',
    },
    {
        value: 'google',
        label: 'Google',
        desc: 'Gemini 2.0, 1.5 Pro...',
        apiUrl: 'https://aistudio.google.com/apikey',
        emoji: '🔵',
    },
    {
        value: 'xai',
        label: 'xAI',
        desc: 'Grok 3, Grok 2...',
        apiUrl: 'https://console.x.ai/team/default/api-keys',
        emoji: '⚫',
    },
    {
        value: 'openrouter',
        label: 'OpenRouter',
        desc: '統一入口，支援數百個模型',
        apiUrl: 'https://openrouter.ai/settings/keys',
        emoji: '🔀',
    },
    {
        value: 'ollama',
        label: 'Ollama',
        desc: '本地執行，無需 API Key',
        defaultBaseUrl: 'http://localhost:11434',
        emoji: '🦙',
        local: true,
    },
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

// ── Popular models per provider ──────────────────────────
// Last updated: 2026-04-08 via official provider docs
const POPULAR_MODELS: Record<string, { value: string; label: string }[]> = {
    openai: [
        { value: 'gpt-4.1', label: 'GPT-4.1' },
        { value: 'gpt-4.1-mini', label: 'GPT-4.1 mini' },
        { value: 'gpt-4.1-nano', label: 'GPT-4.1 nano' },
        { value: 'gpt-4o', label: 'GPT-4o' },
        { value: 'gpt-4o-mini', label: 'GPT-4o mini' },
        { value: 'o3', label: 'o3' },
        { value: 'o4-mini', label: 'o4-mini' },
        { value: 'o3-mini', label: 'o3-mini' },
    ],
    anthropic: [
        { value: 'claude-opus-4-6', label: 'Claude Opus 4.6' },
        { value: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
        { value: 'claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5' },
        { value: 'claude-3-5-sonnet-20241022', label: 'Claude 3.5 Sonnet' },
        { value: 'claude-3-5-haiku-20241022', label: 'Claude 3.5 Haiku' },
    ],
    google: [
        { value: 'gemini-3.1-pro-preview', label: 'Gemini 3.1 Pro (Preview)' },
        { value: 'gemini-3-flash-preview', label: 'Gemini 3 Flash (Preview)' },
        { value: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro' },
        { value: 'gemini-2.5-flash', label: 'Gemini 2.5 Flash' },
        { value: 'gemini-2.0-flash', label: 'Gemini 2.0 Flash' },
        { value: 'gemini-2.0-flash-lite', label: 'Gemini 2.0 Flash Lite' },
    ],
    xai: [
        { value: 'grok-3-beta', label: 'Grok 3' },
        { value: 'grok-3-mini-beta', label: 'Grok 3 mini' },
    ],
    openrouter: [
        { value: 'openai/gpt-4.1', label: 'GPT-4.1 (via OpenRouter)' },
        { value: 'openai/gpt-4o', label: 'GPT-4o (via OpenRouter)' },
        { value: 'anthropic/claude-3-5-sonnet', label: 'Claude 3.5 Sonnet (via OpenRouter)' },
        { value: 'google/gemini-2.5-pro-preview', label: 'Gemini 2.5 Pro (via OpenRouter)' },
        { value: 'google/gemini-2.5-flash-preview', label: 'Gemini 2.5 Flash (via OpenRouter)' },
        { value: 'meta-llama/llama-3.3-70b-instruct', label: 'Llama 3.3 70B (via OpenRouter)' },
        { value: 'deepseek/deepseek-r1', label: 'DeepSeek R1 (via OpenRouter)' },
    ],
    ollama: [
        { value: 'gpt-oss:20b', label: 'GPT-OSS 20B' },
        { value: 'gpt-oss:120b', label: 'GPT-OSS 120B' },
        { value: 'gemma4:e4b', label: 'Gemma 4 (e4b)' },
        { value: 'gemma4:26b', label: 'Gemma 4 26B' },
        { value: 'gemma3', label: 'Gemma 3' },
        { value: 'qwen3', label: 'Qwen3' },
        { value: 'qwen3.5:9b', label: 'Qwen 3.5 9B' },
        { value: 'qwen3.5:27b', label: 'Qwen 3.5 27B' },
        { value: 'llama3.3', label: 'Llama 3.3' },
        { value: 'deepseek-r1', label: 'DeepSeek-R1' },
        { value: 'mistral', label: 'Mistral' },
        { value: 'nomic-embed-text', label: 'nomic-embed-text (embedding)' },
        { value: 'mxbai-embed-large', label: 'mxbai-embed-large (embedding)' },
        { value: 'bge-m3', label: 'BGE-M3 (embedding)' },
    ],
    local: [
        { value: 'multilingual-e5-small', label: 'Multilingual E5 Small' },
    ],
};

// ── ModelComboField: popular presets + free text ─────────
const ModelComboField: React.FC<{ value: string; onChange: (v: string) => void; provider: string; className?: string }> = ({ value, onChange, provider, className }) => {
    const popular = POPULAR_MODELS[provider] ?? [];
    const [open, setOpen] = React.useState(false);
    const ref = React.useRef<HTMLDivElement>(null);

    // Reset input when provider changes
    React.useEffect(() => {
        const nowPreset = (POPULAR_MODELS[provider] ?? []).some(m => m.value === value);
        if (!nowPreset) onChange('');
    }, [provider]);

    // Close dropdown on outside click
    React.useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

    if (popular.length === 0) {
        return <InputField value={value} onChange={onChange} placeholder="model-name" className={className} />;
    }

    // const displayLabel = popular.find(m => m.value === value)?.label ?? value;


    return (
        <div ref={ref} className={`relative ${className ?? ''}`}>
            <div className="flex items-center bg-surface-base border border-stroke-divider rounded-lg focus-within:ring-1 focus-within:ring-accent-default overflow-hidden">
                <input
                    type="text"
                    value={value}
                    onChange={e => onChange(e.target.value)}
                    onFocus={() => setOpen(true)}
                    placeholder="選擇或輸入模型名稱"
                    className="flex-1 bg-transparent px-3 py-1.5 text-fs-sm text-text-primary placeholder:text-text-tertiary focus:outline-none min-w-0"
                />
                <button
                    type="button"
                    onMouseDown={e => { e.preventDefault(); setOpen(o => !o); }}
                    className="px-2 py-1.5 text-text-tertiary hover:text-text-primary border-l border-stroke-divider transition-colors"
                >
                    <svg className="w-3.5 h-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
                        <path d="M2 4l4 4 4-4" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                </button>
            </div>
            {open && (
                <div className="absolute z-50 mt-1 w-full bg-surface-base border border-stroke-divider rounded-lg shadow-lg overflow-hidden">
                    {popular.map(m => (
                        <button
                            key={m.value}
                            type="button"
                            onMouseDown={e => { e.preventDefault(); onChange(m.value); setOpen(false); }}
                            className={`w-full text-left px-3 py-2 text-fs-sm transition-colors hover:bg-surface-subtle ${value === m.value ? 'text-accent-default bg-accent-default/5' : 'text-text-primary'}`}
                        >
                            {m.label}
                            {value === m.value && <span className="float-right text-accent-default">✓</span>}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
};

// ── SummaryModelField: follow presets + free text ────────
const SUMMARY_PRESETS = [
    { value: 'follow_chat', label: '跟隨聊天模型' },
    { value: 'follow_content_processor', label: '跟隨內容處理模型' },
];

const SummaryModelField: React.FC<{ value: string; onChange: (v: string) => void }> = ({ value, onChange }) => {
    const [open, setOpen] = React.useState(false);
    const ref = React.useRef<HTMLDivElement>(null);

    React.useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

    const displayLabel = SUMMARY_PRESETS.find(p => p.value === value)?.label;

    return (
        <div ref={ref} className="relative w-48">
            <div className="flex items-center bg-surface-base border border-stroke-divider rounded-lg focus-within:ring-1 focus-within:ring-accent-default overflow-hidden">
                <input
                    type="text"
                    value={value}
                    onChange={e => onChange(e.target.value)}
                    onFocus={() => setOpen(true)}
                    placeholder={displayLabel ?? '選擇或輸入模型'}
                    className="flex-1 bg-transparent px-3 py-1.5 text-fs-sm text-text-primary placeholder:text-text-tertiary focus:outline-none min-w-0"
                />
                <button
                    type="button"
                    onMouseDown={e => { e.preventDefault(); setOpen(o => !o); }}
                    className="px-2 py-1.5 text-text-tertiary hover:text-text-primary border-l border-stroke-divider transition-colors"
                >
                    <svg className="w-3.5 h-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
                        <path d="M2 4l4 4 4-4" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                </button>
            </div>
            {open && (
                <div className="absolute z-50 mt-1 w-full bg-surface-base border border-stroke-divider rounded-lg shadow-lg overflow-hidden">
                    {SUMMARY_PRESETS.map(p => (
                        <button
                            key={p.value}
                            type="button"
                            onMouseDown={e => { e.preventDefault(); onChange(p.value); setOpen(false); }}
                            className={`w-full text-left px-3 py-2 text-fs-sm transition-colors hover:bg-surface-subtle ${value === p.value ? 'text-accent-default bg-accent-default/5' : 'text-text-primary'}`}
                        >
                            {p.label}
                            {value === p.value && <span className="float-right text-accent-default">✓</span>}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
};

// ── Main Component ───────────────────────────────────────

export const SettingsPage: React.FC = () => {
    const t = useT();
    const [activeTab, setActiveTab] = useState<SettingsTab>('general');
    const [settings, setSettings] = useState<AllSettings | null>(null);
    const [saving, setSaving] = useState(false);
    // const [externalKbs, setExternalKbs] = useState<ExternalKnowledgeBase[]>([]);
    // const [kbLoading, setKbLoading] = useState(false);

    // Rebuild source tags progress
    const [rebuildTagsProgress, setRebuildTagsProgress] = useState<{ current: number; total: number } | null>(null);
    const rebuildTagsUnlistenRef = useRef<(() => void) | null>(null);

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

    // Personal settings state
    const [passwordData, setPasswordData] = useState({ oldPassword: '', newPassword: '', confirmPassword: '' });
    const [personalLoading, setPersonalLoading] = useState(false);
    const [newRecoveryModal, setNewRecoveryModal] = useState<{ open: boolean; mnemonic: string; confirmed: boolean }>({ open: false, mnemonic: '', confirmed: false });
    const [showPasswords, setShowPasswords] = useState<Record<string, boolean>>({});
    const [verifyModal, setVerifyModal] = useState<{
        open: boolean;
        password: string;
        onVerified: () => void;
        loading: boolean;
        error: string;
        title: string;
    }>({ open: false, password: '', onVerified: () => { }, loading: false, error: '', title: '' });

    // ── Load settings ────────────────────────────────────

    useEffect(() => {
        loadSettings();
    }, []);


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
    /*
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
    */

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

                <SectionCard title={t('settings.reminders')}>
                    <SettingRow label={t('settings.reminders_enabled')}>
                        <Toggle checked={settings.reminders?.enabled ?? true} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.enabled = v; })} />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_daily_time')}>
                        <InputField type="time" value={settings.reminders?.dailyReminderTime ?? '09:00'} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.dailyReminderTime = v; })} className="w-32" />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_test_pipeline')}>
                        <button
                            onClick={async () => {
                                try {
                                    const msg = await invoke<string>('trigger_test_reminder');
                                    toast.success(msg);
                                } catch (e) {
                                    toast.error(String(e));
                                }
                            }}
                            className="px-3 py-1.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-xs hover:text-accent-default hover:border-accent-default/30 transition-colors"
                        >
                            {t('settings.reminders_test_trigger')}
                        </button>
                    </SettingRow>
                </SectionCard>

                <SectionCard title={t('settings.reminders_quiet_hours')}>
                    <SettingRow label={t('settings.reminders_quiet_start')}>
                        <InputField type="time" value={settings.reminders?.quietHoursStart ?? '22:00'} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.quietHoursStart = v; })} className="w-32" />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_quiet_end')}>
                        <InputField type="time" value={settings.reminders?.quietHoursEnd ?? '08:00'} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.quietHoursEnd = v; })} className="w-32" />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_weekend_quiet')}>
                        <Toggle checked={settings.reminders?.weekendQuiet ?? false} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.weekendQuiet = v; })} />
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

                <SectionCard title={t('settings.ai_prompt_instruction_title')} desc={t('settings.ai_prompt_instruction_desc')}>
                    <textarea
                        value={settings.chatPromptInstruction}
                        onChange={e => updateSettings(s => { s.chatPromptInstruction = e.target.value; })}
                        placeholder={t('settings.ai_prompt_instruction_placeholder')}
                        rows={4}
                        className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-none focus:outline-none focus:ring-1 focus:ring-accent-default"
                    />
                </SectionCard>

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
                        <div className="bg-surface-base rounded-lg p-4 border border-accent-default/30 space-y-4">
                            {/* Provider 選擇卡片 */}
                            <div>
                                <label className="text-fs-xs text-text-secondary mb-2 block font-medium">{t('settings.provider_type')}</label>
                                <div className="grid grid-cols-3 gap-2">
                                    {PROVIDER_CARDS.map(card => {
                                        const isSelected = editingProfile.provider === card.value;
                                        return (
                                            <button
                                                key={card.value}
                                                type="button"
                                                onClick={() => setEditingProfile({
                                                    ...editingProfile,
                                                    provider: card.value,
                                                    baseUrl: card.defaultBaseUrl ?? '',
                                                })}
                                                className={`flex flex-col items-start gap-1 rounded-xl p-3 border text-left transition-all ${isSelected
                                                    ? 'border-accent-default bg-accent-default/8 shadow-sm'
                                                    : 'border-stroke-divider hover:border-accent-default/40 hover:bg-surface-subtle'
                                                    }`}
                                            >
                                                <div className="flex items-center justify-between w-full">
                                                    <span className="text-base leading-none">{card.emoji}</span>
                                                    {card.apiUrl && (
                                                        <a
                                                            href={card.apiUrl}
                                                            target="_blank"
                                                            rel="noopener noreferrer"
                                                            onClick={e => e.stopPropagation()}
                                                            className="text-text-tertiary hover:text-accent-default transition-colors"
                                                            title="取得 API Key"
                                                        >
                                                            <ExternalLink className="w-3 h-3" />
                                                        </a>
                                                    )}
                                                </div>
                                                <div className={`text-fs-sm font-semibold ${isSelected ? 'text-accent-default' : 'text-text-primary'}`}>{card.label}</div>
                                                <div className="text-fs-xs text-text-tertiary leading-snug">{card.desc}</div>
                                                {card.local && (
                                                    <span className="text-fs-xs text-green-500 bg-green-500/10 px-1.5 py-0.5 rounded-full">本地</span>
                                                )}
                                            </button>
                                        );
                                    })}
                                </div>
                            </div>

                            {/* 名稱 */}
                            <div>
                                <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_name')}</label>
                                <InputField value={editingProfile.name} onChange={v => setEditingProfile({ ...editingProfile, name: v })} placeholder="例: My OpenAI" className="w-full" />
                            </div>

                            {/* Base URL（Ollama 或自訂） */}
                            {(editingProfile.provider === 'ollama' || editingProfile.baseUrl) && (
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_base_url')}</label>
                                    <InputField value={editingProfile.baseUrl ?? ''} onChange={v => setEditingProfile({ ...editingProfile, baseUrl: v })} placeholder="http://localhost:11434" className="w-full" />
                                </div>
                            )}

                            {/* API Key（非 Ollama） */}
                            {editingProfile.provider !== 'ollama' && (
                                <div>
                                    <div className="flex items-center justify-between mb-1">
                                        <label className="text-fs-xs text-text-secondary">{t('settings.provider_apikey')}</label>
                                        {(() => {
                                            const card = PROVIDER_CARDS.find(c => c.value === editingProfile.provider);
                                            return card?.apiUrl ? (
                                                <a
                                                    href={card.apiUrl}
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    className="flex items-center gap-1 text-fs-xs text-accent-default hover:underline"
                                                >
                                                    <ExternalLink className="w-3 h-3" />
                                                    取得 API Key
                                                </a>
                                            ) : null;
                                        })()}
                                    </div>
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
                                <button onClick={handleSaveProfile} className="bg-accent-default text-white px-4 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors">{t('common.save')}</button>
                            </div>
                        </div>
                    )}
                </SectionCard>

                <SectionCard title={t('settings.model_config_title')} desc={t('settings.model_config_desc')}>
                    {renderModelField(t('settings.model_chat'), t('settings.model_chat_desc'), ai.chatLlm, m => updateSettings(s => { s.aiModels.chatLlm = m; }))}
                    {renderModelField(t('settings.model_processor'), t('settings.model_processor_desc'), ai.contentProcessorLlm, m => updateSettings(s => { s.aiModels.contentProcessorLlm = m; }))}
                    {renderModelField(t('settings.model_vision'), t('settings.model_vision_desc'), ai.visionModel, m => updateSettings(s => { s.aiModels.visionModel = m; }))}
                    {renderModelField(t('settings.model_embedding'), t('settings.model_embedding_desc'), ai.embeddingModel, m => updateSettings(s => { s.aiModels.embeddingModel = m; }), true)}
                </SectionCard>

                <SectionCard title={t('settings.model_summary_section')}>
                    <SettingRow label={t('settings.model_summary')} desc={t('settings.model_summary_desc')}>
                        <SummaryModelField
                            value={ai.summaryModel ?? 'follow_chat'}
                            onChange={v => updateSettings(s => { s.aiModels.summaryModel = v; })}
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



    const handleTestModel = async (model: ModelSettings) => {
        if (!model.provider || !model.model) {
            toast.error('請先選擇供應商與模型');
            return;
        }

        const tid = toast.loading(`正在測試模型 ${model.model}...`);

        let baseUrl = undefined;
        let apiKey = undefined;

        // 尋找對應的 profile
        const profile = settings?.aiModels.providerProfiles.find(p => p.provider === model.provider);
        if (profile) {
            baseUrl = profile.baseUrl;
            apiKey = profile.apiKey;
        } else if (model.provider === 'ollama') {
            baseUrl = 'http://localhost:11434';
        }

        try {
            const res = await invoke<string>('test_model_connection', {
                provider: model.provider,
                model: model.model,
                baseUrl: baseUrl,
                apiKey: apiKey
            });
            toast.success(`連線成功: 收到回應 "${res}"`, { id: tid });
        } catch (e: any) {
            toast.error(`連線失敗: ${e.toString()}`, { id: tid });
        }
    };

    const renderModelField = (label: string, desc: string, model: ModelSettings, onChange: (m: ModelSettings) => void, includeLocal?: boolean) => {
        const profiles = settings?.aiModels.providerProfiles ?? [];
        const providerOptions = profiles.map(p => ({
            value: p.provider,
            label: p.name || PROVIDER_OPTIONS.find(o => o.value === p.provider)?.label || p.provider,
        }));
        // 去重（同 provider 可能有多個 profile，只保留一個選項）
        const seen = new Set<string>();
        const uniqueOptions = providerOptions.filter(o => {
            if (seen.has(o.value)) return false;
            seen.add(o.value);
            return true;
        });
        if (includeLocal) {
            uniqueOptions.push({ value: 'local', label: t('settings.local_provider') });
        }
        return (
            <div className="bg-surface-base rounded-lg px-4 py-3 border border-stroke-divider space-y-2">
                <div className="text-fs-sm font-medium text-text-primary">{label}</div>
                <div className="text-fs-xs text-text-tertiary">{desc}</div>
                <div className="grid grid-cols-2 gap-3 pt-1">
                    <div>
                        <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_type')}</label>
                        <SelectField
                            value={model.provider}
                            onChange={v => {
                                const popular = POPULAR_MODELS[v] ?? [];
                                const newModel = popular.length > 0 ? popular[0].value : '';
                                onChange({ ...model, provider: v, model: newModel });
                            }}
                            options={uniqueOptions}
                            className="w-full"
                        />
                    </div>
                    <div>
                        <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.model_name')}</label>
                        <div className="flex gap-2 items-start">
                            <ModelComboField value={model.model} onChange={v => onChange({ ...model, model: v })} provider={model.provider} className="flex-1" />
                            <button
                                onClick={() => handleTestModel(model)}
                                className="px-3 py-1.5 mt-0.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-sm hover:text-accent-default hover:border-accent-default/30 transition-colors shrink-0"
                            >
                                測試
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        );
    };

    const renderAI = () => {
        if (!settings) return null;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.ai_section_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.ai_section_desc')}</p>
                </div>

                <SectionCard title={t('settings.telegram_override_title')} desc={t('settings.telegram_override_desc')}>
                    <textarea
                        value={settings.telegram.promptInstructionOverride || ''}
                        onChange={e => updateSettings(s => { s.telegram.promptInstructionOverride = e.target.value; })}
                        placeholder={t('settings.ai_prompt_instruction_placeholder')}
                        rows={2}
                        className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-none focus:outline-none focus:ring-1 focus:ring-accent-default"
                    />
                </SectionCard>

                {/* ── Telegram Bot ─────────────────────────────────── */}
                <SectionCard
                    title="Telegram Bot"
                    desc={t('settings.telegram_desc')}
                    action={
                        <button
                            onClick={() => openUrl('https://t.me/BotFather')}
                            className="flex items-center gap-1.5 text-fs-sm text-accent-default hover:text-accent-light1 transition-colors"
                        >
                            <ExternalLink size={13} />
                            BotFather
                        </button>
                    }
                >
                    <SettingRow label={t('settings.telegram_enabled')} desc={t('settings.telegram_enabled_desc')}>
                        <Toggle checked={settings.telegram.enabled} onChange={v => updateSettings(s => { s.telegram.enabled = v; })} />
                    </SettingRow>
                    {settings.telegram.enabled && (
                        <>
                            <SettingRow label="Bot Token" desc={t('settings.telegram_token_desc')}>
                                <div className="flex items-center gap-2">
                                    <InputField
                                        value={settings.telegram.botToken}
                                        onChange={v => updateSettings(s => { s.telegram.botToken = v; })}
                                        type={showPasswords.botToken ? 'text' : 'password'}
                                        className="w-72"
                                        placeholder="123456:ABC-DEF..."
                                    />
                                    <button
                                        onClick={() => setShowPasswords(prev => ({ ...prev, botToken: !prev.botToken }))}
                                        className="p-1.5 text-text-tertiary hover:text-text-secondary transition-colors"
                                    >
                                        {showPasswords.botToken ? <EyeOff size={14} /> : <Eye size={14} />}
                                    </button>
                                </div>
                            </SettingRow>
                            <SettingRow label="Allowed User IDs" desc={t('settings.telegram_userids_desc')}>
                                <div className="flex items-center gap-2">
                                    <InputField
                                        value={settings.telegram.allowedUserIds.join(', ')}
                                        onChange={v => updateSettings(s => {
                                            s.telegram.allowedUserIds = v.split(',').map(id => parseInt(id.trim())).filter(id => !isNaN(id));
                                        })}
                                        className="w-72"
                                        placeholder="123456789, 987654321"
                                    />
                                    <button
                                        onClick={async () => {
                                            try {
                                                const ids = await invoke<number[]>('telegram_get_allowed_user_ids', { botToken: settings.telegram.botToken });
                                                updateSettings(s => { s.telegram.allowedUserIds = ids; });
                                                toast.success(`${t('settings.telegram_fetch_success')}: ${ids.join(', ')}`);
                                            } catch (e) {
                                                toast.error(String(e));
                                            }
                                        }}
                                        disabled={!settings.telegram.botToken}
                                        className="px-3 py-1.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-xs hover:text-accent-default hover:border-accent-default/30 transition-colors disabled:opacity-50 shrink-0"
                                    >
                                        {t('settings.telegram_fetch_chatid')}
                                    </button>
                                    <div className="w-px h-4 bg-stroke-divider mx-1" />
                                    <button
                                        onClick={async () => {
                                            try {
                                                await invoke('test_telegram_notification', {
                                                    botToken: settings.telegram.botToken,
                                                    userIds: settings.telegram.allowedUserIds
                                                });
                                                toast.success(t('settings.telegram_test_success'));
                                            } catch (e) {
                                                toast.error(String(e));
                                            }
                                        }}
                                        disabled={!settings.telegram.botToken || settings.telegram.allowedUserIds.length === 0}
                                        className="px-3 py-1.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-xs hover:text-accent-default hover:border-accent-default/30 transition-colors disabled:opacity-50 shrink-0"
                                    >
                                        {t('settings.telegram_test_send')}
                                    </button>
                                </div>
                            </SettingRow>
                            <div className="mt-3 p-3 bg-surface-1 rounded-lg">
                                <p className="text-fs-xs text-text-tertiary leading-relaxed">
                                    {t('settings.telegram_help')}
                                </p>
                            </div>
                        </>
                    )}
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
                            disabled={rebuildTagsProgress !== null}
                            onClick={async () => {
                                const cleanup = () => {
                                    setRebuildTagsProgress(null);
                                    if (rebuildTagsUnlistenRef.current) {
                                        rebuildTagsUnlistenRef.current();
                                        rebuildTagsUnlistenRef.current = null;
                                    }
                                };
                                // 訂閱進度事件
                                const unlisten = await listen<{ current: number; total: number; done: boolean }>(
                                    'rebuild-tags-progress',
                                    (event) => {
                                        const { current, total, done } = event.payload;
                                        if (done) {
                                            cleanup();
                                        } else {
                                            setRebuildTagsProgress({ current, total });
                                        }
                                    }
                                );
                                rebuildTagsUnlistenRef.current = unlisten;
                                // 設初始 loading 狀態（避免 listener 在 invoke 前就清掉）
                                setRebuildTagsProgress({ current: 0, total: 0 });
                                try {
                                    const count: number = await invoke('rebuild_source_tags');
                                    toast.success(`${t('settings.rebuild_source_tags')} ${t('common.success')} (${count})`);
                                } catch (e: any) {
                                    toast.error(`${t('common.error')}: ${e.toString()}`);
                                } finally {
                                    cleanup();
                                }
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors disabled:opacity-60 disabled:cursor-not-allowed"
                        >
                            <RefreshCw className={`w-4 h-4 text-accent-default ${rebuildTagsProgress ? 'animate-spin' : ''}`} />
                            {rebuildTagsProgress
                                ? `${rebuildTagsProgress.current} / ${rebuildTagsProgress.total}`
                                : t('settings.rebuild_source_tags')
                            }
                        </button>
                        <button
                            onClick={() => {
                                setVerifyModal({
                                    open: true,
                                    password: '',
                                    title: t('settings.export_kb'),
                                    loading: false,
                                    error: '',
                                    onVerified: async () => {
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
                                    }
                                });
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Upload className="w-4 h-4 text-accent-default" />
                            {t('settings.export_kb')}
                        </button>
                        <button
                            onClick={() => {
                                setVerifyModal({
                                    open: true,
                                    password: '',
                                    title: t('settings.import_kb'),
                                    loading: false,
                                    error: '',
                                    onVerified: async () => {
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
                                    }
                                });
                            }}
                            className="flex items-center gap-2 bg-surface-base border border-stroke-divider rounded-lg px-4 py-3 text-sm text-text-primary hover:bg-surface-subtle transition-colors"
                        >
                            <Download className="w-4 h-4 text-accent-default" />
                            {t('settings.import_kb')}
                        </button>
                        <button
                            onClick={() => {
                                setVerifyModal({
                                    open: true,
                                    password: '',
                                    title: t('settings.delete_kb'),
                                    loading: false,
                                    error: '',
                                    onVerified: async () => {
                                        if (!window.confirm(t('common.warning_irreversible'))) return;
                                        const tid = toast.loading(t('common.loading'));
                                        try {
                                            await invoke('delete_kb');
                                            toast.success(t('common.success'), { id: tid });
                                        } catch (e: any) {
                                            toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
                                        }
                                    }
                                });
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

                {/* 
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
                */}

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

                <SectionCard title={t('settings.editor_override_title')} desc={t('settings.editor_override_desc')}>
                    <textarea
                        value={settings.editor.promptInstructionOverride || ''}
                        onChange={e => updateSettings(s => { s.editor.promptInstructionOverride = e.target.value; })}
                        placeholder={t('settings.ai_prompt_instruction_placeholder')}
                        rows={2}
                        className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-none focus:outline-none focus:ring-1 focus:ring-accent-default"
                    />
                </SectionCard>

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

                <div className="flex justify-end mt-6">
                    <button onClick={() => saveSettings()} disabled={saving} className="bg-accent-default text-white px-5 py-2 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50">
                        {saving ? t('common.saving') : t('common.save')}
                    </button>
                </div>
            </div>
        );
    };

    // ── Tab: 個人設定 ────────────────────────────────────

    const handlePasswordChange = async () => {
        if (!settings) return;
        if (!passwordData.oldPassword || !passwordData.newPassword) {
            toast.error('請輸入密碼');
            return;
        }
        if (passwordData.newPassword !== passwordData.confirmPassword) {
            toast.error(t('auth.setup.password_mismatch'));
            return;
        }
        if (passwordData.newPassword.length < 8) {
            toast.error(t('auth.setup.password_too_short'));
            return;
        }

        setPersonalLoading(true);
        const tid = toast.loading('正在更改密碼...');
        try {
            const mnemonic = await invoke<string>('change_password', {
                payload: {
                    oldPassword: passwordData.oldPassword,
                    newPassword: passwordData.newPassword,
                    kbPath: settings.knowledge.kbPath
                }
            });
            toast.success(t('settings.password_change_success'), { id: tid });
            setPasswordData({ oldPassword: '', newPassword: '', confirmPassword: '' });
            setNewRecoveryModal({ open: true, mnemonic, confirmed: false });
        } catch (e: any) {
            toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
        } finally {
            setPersonalLoading(false);
        }
    };

    const handleRegenerateRecovery = async () => {
        if (!settings) return;
        const password = window.prompt(t('auth.login.password_placeholder'));
        if (!password) return;

        if (!window.confirm(t('settings.regenerate_confirm'))) return;

        setPersonalLoading(true);
        const tid = toast.loading('正在生成新恢復碼...');
        try {
            const mnemonic = await invoke<string>('reset_recovery_phrase', {
                payload: {
                    password,
                    kbPath: settings.knowledge.kbPath
                }
            });
            toast.success(t('settings.regenerate_success'), { id: tid });
            setNewRecoveryModal({ open: true, mnemonic, confirmed: false });
        } catch (e: any) {
            toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
        } finally {
            setPersonalLoading(false);
        }
    };

    const renderPersonal = () => {
        if (!settings) return null;
        return (
            <div className="max-w-4xl mx-auto">
                <div className="mb-8">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.personal_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.personal_desc')}</p>
                </div>

                <SectionCard title={t('settings.security_section')}>
                    <div className="space-y-6">
                        <div className="p-4 bg-surface-base rounded-lg border border-stroke-divider">
                            <h5 className="text-fs-sm font-semibold text-text-primary mb-4 flex items-center gap-2">
                                <KeyRound className="w-4 h-4 text-accent-default" />
                                {t('settings.change_password')}
                            </h5>
                            <div className="space-y-4">
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div className="space-y-1.5">
                                        <label className="text-fs-xs text-text-secondary">{t('settings.current_password')}</label>
                                        <div className="relative">
                                            <input
                                                type={showPasswords.old ? "text" : "password"}
                                                value={passwordData.oldPassword}
                                                onChange={e => setPasswordData(d => ({ ...d, oldPassword: e.target.value }))}
                                                className="w-full bg-surface-layer border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm focus:outline-none focus:ring-1 focus:ring-accent-default"
                                            />
                                            <button onClick={() => setShowPasswords(p => ({ ...p, old: !p.old }))} className="absolute right-2 top-1/2 -translate-y-1/2 text-text-tertiary">
                                                {showPasswords.old ? <EyeOff size={14} /> : <Eye size={14} />}
                                            </button>
                                        </div>
                                    </div>
                                    <div />
                                    <div className="space-y-1.5">
                                        <label className="text-fs-xs text-text-secondary">{t('settings.new_password')}</label>
                                        <div className="relative">
                                            <input
                                                type={showPasswords.new ? "text" : "password"}
                                                value={passwordData.newPassword}
                                                onChange={e => setPasswordData(d => ({ ...d, newPassword: e.target.value }))}
                                                className="w-full bg-surface-layer border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm focus:outline-none focus:ring-1 focus:ring-accent-default"
                                            />
                                            <button onClick={() => setShowPasswords(p => ({ ...p, new: !p.new }))} className="absolute right-2 top-1/2 -translate-y-1/2 text-text-tertiary">
                                                {showPasswords.new ? <EyeOff size={14} /> : <Eye size={14} />}
                                            </button>
                                        </div>
                                    </div>
                                    <div className="space-y-1.5">
                                        <label className="text-fs-xs text-text-secondary">{t('settings.confirm_new_password')}</label>
                                        <input
                                            type="password"
                                            value={passwordData.confirmPassword}
                                            onChange={e => setPasswordData(d => ({ ...d, confirmPassword: e.target.value }))}
                                            className="w-full bg-surface-layer border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm focus:outline-none focus:ring-1 focus:ring-accent-default"
                                        />
                                    </div>
                                </div>
                                <div className="flex justify-start">
                                    <button
                                        onClick={handlePasswordChange}
                                        disabled={personalLoading}
                                        className="bg-accent-default text-white px-4 py-1.5 rounded-lg text-fs-sm hover:bg-accent-light1 transition-colors disabled:opacity-50"
                                    >
                                        {t('settings.change_password')}
                                    </button>
                                </div>
                            </div>
                        </div>

                        <div className="p-4 bg-surface-base rounded-lg border border-stroke-divider">
                            <h5 className="text-fs-sm font-semibold text-text-primary mb-2 flex items-center gap-2">
                                <ShieldCheck className="w-4 h-4 text-accent-default" />
                                {t('settings.recovery_code_section')}
                            </h5>
                            <p className="text-fs-xs text-text-tertiary mb-4">
                                {t('settings.recovery_code_desc')}
                            </p>
                            <button
                                onClick={handleRegenerateRecovery}
                                disabled={personalLoading}
                                className="border border-stroke-divider text-text-primary px-4 py-1.5 rounded-lg text-fs-sm hover:bg-surface-subtle transition-colors disabled:opacity-50"
                            >
                                {t('settings.regenerate_recovery_code')}
                            </button>
                        </div>
                    </div>
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
                {activeTab === 'personal' && renderPersonal()}
                {activeTab === 'provider' && renderProvider()}
                {activeTab === 'ai' && renderAI()}
                {activeTab === 'knowledge' && renderKnowledge()}

                {activeTab === 'other' && renderOther()}
            </div>

            {/* ── 匯出備份恢復碼 Modal ── */}
            {exportModal.open && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
                    <div className="bg-surface-base border border-stroke-divider rounded-xl p-6 w-[480px] max-w-full shadow-xl">
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
                    <div className="bg-surface-base border border-stroke-divider rounded-xl p-6 w-[480px] max-w-full shadow-xl">
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

            {/* ── 密碼驗證 Modal ── */}
            {verifyModal.open && (
                <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
                    <div className="bg-surface-base border border-stroke-divider rounded-2xl p-8 w-[400px] max-w-full shadow-2xl animate-in fade-in zoom-in duration-200">
                        <div className="flex items-center gap-3 mb-6">
                            <div className="p-2 bg-accent-default/10 rounded-full">
                                <KeyRound className="w-6 h-6 text-accent-default" />
                            </div>
                            <h3 className="text-fs-xl font-bold text-text-primary">{verifyModal.title}</h3>
                        </div>

                        <p className="text-fs-sm text-text-secondary mb-6 leading-relaxed">
                            {t('settings.verify_password_desc')}
                        </p>

                        <div className="space-y-4 mb-8">
                            <div className="space-y-1.5">
                                <label className="text-fs-xs text-text-secondary font-medium">{t('auth.login.password_label')}</label>
                                <div className="relative">
                                    <input
                                        autoFocus
                                        type={showPasswords.verify ? "text" : "password"}
                                        value={verifyModal.password}
                                        onChange={e => setVerifyModal(s => ({ ...s, password: e.target.value, error: '' }))}
                                        onKeyDown={e => e.key === 'Enter' && !verifyModal.loading && (e.currentTarget.parentElement?.parentElement?.parentElement?.nextElementSibling?.children[1] as HTMLButtonElement)?.click()}
                                        className="w-full bg-surface-layer border border-stroke-divider rounded-xl px-4 py-2.5 text-fs-sm focus:outline-none focus:ring-2 focus:ring-accent-default/20 focus:border-accent-default transition-all"
                                        placeholder={t('auth.login.password_placeholder')}
                                    />
                                    <button
                                        type="button"
                                        onClick={() => setShowPasswords(p => ({ ...p, verify: !p.verify }))}
                                        className="absolute right-3 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary transition-colors"
                                    >
                                        {showPasswords.verify ? <EyeOff size={16} /> : <Eye size={16} />}
                                    </button>
                                </div>
                            </div>
                            {verifyModal.error && (
                                <p className="text-fs-xs text-red-500 animate-in slide-in-from-top-1">{verifyModal.error}</p>
                            )}
                        </div>

                        <div className="flex gap-3 justify-end">
                            <button
                                onClick={() => setVerifyModal(s => ({ ...s, open: false }))}
                                className="px-5 py-2.5 text-fs-sm font-medium text-text-secondary hover:text-text-primary hover:bg-surface-subtle rounded-xl transition-all"
                            >
                                {t('common.cancel')}
                            </button>
                            <button
                                disabled={verifyModal.loading || !verifyModal.password}
                                onClick={async () => {
                                    setVerifyModal(s => ({ ...s, loading: true, error: '' }));
                                    try {
                                        const ok = await invoke<boolean>('verify_password', {
                                            payload: {
                                                password: verifyModal.password,
                                                kbPath: settings?.knowledge.kbPath
                                            }
                                        });

                                        if (ok) {
                                            const originalCallback = verifyModal.onVerified;
                                            setVerifyModal(s => ({ ...s, open: false }));
                                            originalCallback();
                                        } else {
                                            setVerifyModal(s => ({ ...s, loading: false, error: t('auth.login.invalid_credentials') }));
                                        }
                                    } catch (e: any) {
                                        setVerifyModal(s => ({ ...s, loading: false, error: e.toString() }));
                                    }
                                }}
                                className="px-8 py-2.5 bg-accent-default text-white rounded-xl font-semibold hover:bg-accent-light1 transition-all shadow-lg shadow-accent-default/20 disabled:opacity-50 active:scale-95 flex items-center gap-2"
                            >
                                {verifyModal.loading ? (
                                    <>
                                        <RefreshCw className="w-4 h-4 animate-spin" />
                                        {t('common.verifying')}
                                    </>
                                ) : (
                                    t('common.confirm')
                                )}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* ── 匯出備份恢復碼 Modal ── */}
            {exportModal.open && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
                    <div className="bg-surface-base border border-stroke-divider rounded-xl p-6 w-[480px] max-w-full shadow-xl">
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

            {/* ── 新恢復碼 Modal ── */}
            {newRecoveryModal.open && (
                <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60">
                    <div className="bg-surface-base border border-stroke-divider rounded-2xl p-8 w-[520px] max-w-full shadow-2xl animate-in fade-in zoom-in duration-300">
                        <div className="flex items-center gap-3 mb-6">
                            <div className="p-2 bg-accent-default/10 rounded-full">
                                <ShieldCheck className="w-6 h-6 text-accent-default" />
                            </div>
                            <h3 className="text-fs-xl font-bold text-text-primary">{t('settings.import_success_title')}</h3>
                        </div>

                        <p className="text-fs-sm text-text-secondary mb-6 leading-relaxed">
                            {t('settings.import_success_desc')}
                        </p>

                        <div className="bg-surface-base border border-stone-200/50 dark:border-white/10 rounded-xl p-5 mb-6 font-mono text-fs-sm text-text-primary leading-relaxed break-words shadow-inner bg-accent-default/5 select-all">
                            {newRecoveryModal.mnemonic}
                        </div>

                        <button
                            onClick={() => {
                                navigator.clipboard.writeText(newRecoveryModal.mnemonic);
                                toast.success(t('common.copied'));
                            }}
                            className="flex items-center gap-2 text-fs-sm font-medium text-accent-default hover:text-accent-light1 mb-8 w-fit transition-colors"
                        >
                            <Download className="w-4 h-4" />
                            {t('settings.copy_mnemonic')}
                        </button>

                        <label className="flex items-start gap-3 p-4 bg-surface-subtle/50 rounded-xl border border-stroke-divider cursor-pointer mb-8 hover:bg-surface-subtle transition-colors group">
                            <input
                                type="checkbox"
                                checked={newRecoveryModal.confirmed}
                                onChange={e => setNewRecoveryModal(s => ({ ...s, confirmed: e.target.checked }))}
                                className="mt-1 w-4 h-4 rounded border-stroke-divider text-accent-default focus:ring-accent-default cursor-pointer"
                            />
                            <span className="text-fs-sm text-text-secondary group-hover:text-text-primary transition-colors">
                                {t('settings.save_new_mnemonic_confirm')}
                            </span>
                        </label>

                        <div className="flex justify-end">
                            <button
                                disabled={!newRecoveryModal.confirmed}
                                onClick={async () => {
                                    try {
                                        await invoke('confirm_new_recovery', { kbPath: settings?.knowledge.kbPath });
                                        setNewRecoveryModal({ open: false, mnemonic: '', confirmed: false });
                                        window.location.reload(); // 重啟或重新載入以確保安全狀態更新
                                    } catch (e) {
                                        toast.error('確認失敗');
                                    }
                                }}
                                className="px-8 py-2.5 bg-accent-default text-white rounded-xl font-semibold hover:bg-accent-light1 transition-all shadow-lg shadow-accent-default/20 disabled:opacity-50 disabled:grayscale active:scale-95"
                            >
                                {t('common.confirm')}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
