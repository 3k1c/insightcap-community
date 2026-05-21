import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { useThemeStore, type Theme } from '../stores/themeStore';
import { useLanguageStore } from '../stores/languageStore';
import type { Language } from '../i18n';
import {
    Settings2, Server, Sparkles, BookOpen, PenLine,
    Plus, Trash2, Eye, EyeOff, ExternalLink,
    RefreshCw, Download, Upload, AlertTriangle, Wrench,
    User, ShieldCheck, KeyRound, Save, Clock, GripVertical, ArrowUp, ArrowDown,
} from 'lucide-react';
import { useT } from '../hooks/useT';
import { save } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { toast } from 'sonner';
import {
    applyProviderOptionSelection,
    buildProviderOptions,
    getSelectedProviderOptionValue,
} from './settings-provider-utils';
import {
    cloneDefaultEditorAiActions,
    createEditorAiAction,
    getEnabledEditorAiActions,
    moveEditorAiAction,
    normalizeEditorAiActions,
    reorderEditorAiAction,
    removeEditorAiAction,
    updateEditorAiAction,
    type EditorAiAction,
} from '../lib/editor-ai-actions';
import packageJson from '../../package.json';

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
        speechToTextModel: ModelSettings;
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
        aiActions: EditorAiAction[];
        promptInstructionOverride?: string;
    };
    telegram: {
        botToken: string;
        allowedUserIds: number[];
        enabled: boolean;
        promptInstructionOverride?: string;
    };
    reminders: {
        aiEnabled: boolean;
        enabled: boolean;
        dailyReminderTime: string;
        quietHoursStart: string;
        quietHoursEnd: string;
        weekendQuiet: boolean;
    };
    bilibiliSessdata?: string;
    chatPromptInstruction: string;
    aiUsage?: {
        mode: 'economy' | 'balanced' | 'quality';
    };
}

type SettingsTab = 'general' | 'personal' | 'provider' | 'ai' | 'knowledge' | 'other';

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

const APP_VERSION_FALLBACK = packageJson.version;



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
    local?: boolean;
}

const PROVIDER_CARDS: ProviderCard[] = [
    {
        value: 'openai',
        label: 'OpenAI',
        desc: 'GPT-4o, o1, o3...',
        apiUrl: 'https://platform.openai.com/api-keys',
    },
    {
        value: 'anthropic',
        label: 'Anthropic',
        desc: 'Claude 4, Claude 3.5...',
        apiUrl: 'https://console.anthropic.com/settings/keys',
    },
    {
        value: 'google',
        label: 'Google',
        desc: 'Gemini 2.0, 1.5 Pro...',
        apiUrl: 'https://aistudio.google.com/apikey',
    },
    {
        value: 'xai',
        label: 'xAI',
        desc: 'Grok 3, Grok 2...',
        apiUrl: 'https://console.x.ai/team/default/api-keys',
    },
    {
        value: 'openrouter',
        label: 'OpenRouter',
        desc: 'Unified gateway, supports hundreds of models',
        apiUrl: 'https://openrouter.ai/settings/keys',
    },
    {
        value: 'ollama',
        label: 'Ollama',
        desc: 'Run locally, no API key required',
        defaultBaseUrl: 'http://localhost:11434',
        local: true,
    },
];


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

const TimeField: React.FC<{ value: string; onChange: (v: string) => void; className?: string }> = ({ value, onChange, className }) => {
    const [open, setOpen] = useState(false);
    const rootRef = useRef<HTMLDivElement>(null);
    const [hour = '00', minute = '00'] = value.split(':');
    const hours = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, '0'));
    const minutes = Array.from({ length: 60 }, (_, i) => String(i).padStart(2, '0'));

    useEffect(() => {
        if (!open) return;

        const handlePointerDown = (event: MouseEvent) => {
            if (!rootRef.current?.contains(event.target as Node)) {
                setOpen(false);
            }
        };
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                setOpen(false);
            }
        };

        document.addEventListener('mousedown', handlePointerDown);
        document.addEventListener('keydown', handleKeyDown);
        return () => {
            document.removeEventListener('mousedown', handlePointerDown);
            document.removeEventListener('keydown', handleKeyDown);
        };
    }, [open]);

    const selectTimePart = (nextHour: string, nextMinute: string) => {
        onChange(`${nextHour}:${nextMinute}`);
    };

    return (
        <div ref={rootRef} className={`relative ${className ?? ''}`}>
            <button
                type="button"
                onClick={() => setOpen(v => !v)}
                className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-1.5 text-fs-sm text-text-primary focus:outline-none focus:ring-1 focus:ring-accent-default hover:border-accent-default/50 transition-colors flex items-center justify-between gap-2"
            >
                <span>{hour}:{minute}</span>
                <Clock className="w-4 h-4 text-text-tertiary" />
            </button>

            {open && (
                <div className="absolute right-0 top-full z-50 mt-2 w-40 rounded-xl border border-stroke-divider bg-surface-base shadow-xl p-2">
                    <div className="grid grid-cols-2 gap-2">
                        <div className="max-h-56 overflow-y-auto pr-1">
                            {hours.map(h => (
                                <button
                                    key={h}
                                    type="button"
                                    onClick={() => selectTimePart(h, minute)}
                                    className={`w-full px-3 py-1.5 rounded-lg text-fs-sm transition-colors ${h === hour ? 'bg-accent-default text-white' : 'text-text-primary hover:bg-surface-subtle'}`}
                                >
                                    {h}
                                </button>
                            ))}
                        </div>
                        <div className="max-h-56 overflow-y-auto pl-1 border-l border-stroke-divider">
                            {minutes.map(m => (
                                <button
                                    key={m}
                                    type="button"
                                    onClick={() => selectTimePart(hour, m)}
                                    className={`w-full px-3 py-1.5 rounded-lg text-fs-sm transition-colors ${m === minute ? 'bg-accent-default text-white' : 'text-text-primary hover:bg-surface-subtle'}`}
                                >
                                    {m}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

const HotkeyInput: React.FC<{ value: string; onChange: (v: string) => void; className?: string }> = ({ value, onChange, className }) => {
    const t = useT();
    const [recording, setRecording] = React.useState(false);
    const recordingRef = React.useRef(false);

    const resumeGlobalHotkeys = React.useCallback(() => {
        void invoke('resume_global_hotkeys').catch(e => {
            console.error('Failed to resume global hotkeys', e);
        });
    }, []);

    const stopRecording = React.useCallback(() => {
        if (!recordingRef.current) return;
        recordingRef.current = false;
        setRecording(false);
        resumeGlobalHotkeys();
    }, [resumeGlobalHotkeys]);

    const startRecording = React.useCallback(() => {
        if (recordingRef.current) return;
        recordingRef.current = true;
        setRecording(true);
        void invoke('pause_global_hotkeys').catch(e => {
            console.error('Failed to pause global hotkeys', e);
        });
    }, []);

    React.useEffect(() => {
        return () => {
            if (recordingRef.current) {
                recordingRef.current = false;
                resumeGlobalHotkeys();
            }
        };
    }, [resumeGlobalHotkeys]);

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!recordingRef.current) return;
        e.preventDefault();
        e.stopPropagation();

        if (e.key === 'Escape') {
            stopRecording();
            return;
        }

        if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
            return;
        }

        const mods = [];
        if (e.ctrlKey || e.metaKey) mods.push('CommandOrControl');
        if (e.altKey) mods.push('Alt');
        if (e.shiftKey) mods.push('Shift');

        let key = e.key.toUpperCase();
        if (e.code.startsWith('Key')) key = e.code.replace('Key', '');
        else if (e.code.startsWith('Digit')) key = e.code.replace('Digit', '');
        else if (key === ' ') key = 'Space';

        const hotkey = [...mods, key].join('+');
        onChange(hotkey);
        stopRecording();
    };

    return (
        <button
            type="button"
            className={`text-left px-3 py-1.5 rounded-lg text-fs-sm border transition-colors focus:outline-none w-full ${recording ? 'bg-accent-default/10 border-accent-default text-accent-default' : 'bg-surface-base border-stroke-divider text-text-primary hover:border-accent-default/50'} ${className ?? ''}`}
            onClick={startRecording}
            onKeyDown={handleKeyDown}
            onBlur={stopRecording}
        >
            {recording ? t('settings.hotkey_recording') : (value || t('settings.hotkey_none'))}
        </button>
    );
};

interface ModelOption {
    value: string;
    label: string;
    category?: 'embedding' | 'speech-to-text';
}

const POPULAR_MODELS: Record<string, ModelOption[]> = {
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
        { value: 'gemma4:e4b', label: 'Gemma 4 (e4b)' },
        { value: 'gpt-oss:20b', label: 'GPT-OSS 20B' },
        { value: 'gpt-oss:120b', label: 'GPT-OSS 120B' },
        { value: 'gemma4:26b', label: 'Gemma 4 26B' },
        { value: 'gemma3', label: 'Gemma 3' },
        { value: 'qwen3', label: 'Qwen3' },
        { value: 'qwen3.5:9b', label: 'Qwen 3.5 9B' },
        { value: 'qwen3.5:27b', label: 'Qwen 3.5 27B' },
        { value: 'llama3.3', label: 'Llama 3.3' },
        { value: 'deepseek-r1', label: 'DeepSeek-R1' },
        { value: 'mistral', label: 'Mistral' },
        { value: 'nomic-embed-text', label: 'nomic-embed-text (embedding)', category: 'embedding' },
        { value: 'mxbai-embed-large', label: 'mxbai-embed-large (embedding)', category: 'embedding' },
        { value: 'bge-m3', label: 'BGE-M3 (embedding)', category: 'embedding' },
    ],
    local: [
        { value: 'multilingual-e5-small', label: 'Multilingual E5 Small (embedding)', category: 'embedding' },
        { value: 'tiny', label: 'Whisper Tiny (75 MB)', category: 'speech-to-text' },
        { value: 'base', label: 'Whisper Base (142 MB)', category: 'speech-to-text' },
        { value: 'small', label: 'Whisper Small (466 MB)', category: 'speech-to-text' },
        { value: 'medium', label: 'Whisper Medium (1.5 GB)', category: 'speech-to-text' },
    ],
};

const ModelComboField: React.FC<{
    value: string;
    onChange: (v: string) => void;
    provider: string;
    inputPlaceholder: string;
    className?: string;
    category?: 'embedding' | 'speech-to-text' | 'chat';
}> = ({ value, onChange, provider, inputPlaceholder, className, category = 'chat' }) => {
    const rawPopular = POPULAR_MODELS[provider] ?? [];
    const popular = rawPopular.filter(m => {
        if (category === 'chat') return !m.category;
        return m.category === category;
    });
    const [open, setOpen] = React.useState(false);
    const ref = React.useRef<HTMLDivElement>(null);

    React.useEffect(() => {
        // Find if current value is in the filtered popular list
        const nowPreset = popular.some(m => m.value === value);
        if (!nowPreset && popular.length > 0 && !value) {
            // Only auto-clear if it's empty and we have options, 
            // but actually it's better to NOT auto-clear if the user typed something custom.
            // However, when switching provider, we might want to reset.
        }
    }, [provider, category]);

    React.useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

    if (popular.length === 0) {
        return <InputField value={value} onChange={onChange} placeholder={inputPlaceholder} className={className} />;
    }



    return (
        <div ref={ref} className={`relative ${className ?? ''}`}>
            <div className="flex items-center bg-surface-base border border-stroke-divider rounded-lg focus-within:ring-1 focus-within:ring-accent-default overflow-hidden">
                <input
                    type="text"
                    value={value}
                    onChange={e => onChange(e.target.value)}
                    onFocus={() => setOpen(true)}
                    placeholder={inputPlaceholder}
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
                            {value === m.value && <span className="float-right text-accent-default">v</span>}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
};

const SummaryModelField: React.FC<{
    value: string;
    onChange: (v: string) => void;
    followChatLabel: string;
    followProcessorLabel: string;
    inputPlaceholder: string;
}> = ({ value, onChange, followChatLabel, followProcessorLabel, inputPlaceholder }) => {
    const summaryPresets = [
        { value: 'follow_chat', label: followChatLabel },
        { value: 'follow_content_processor', label: followProcessorLabel },
    ];
    const [open, setOpen] = React.useState(false);
    const ref = React.useRef<HTMLDivElement>(null);

    React.useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);

    const displayLabel = summaryPresets.find(p => p.value === value)?.label;

    return (
        <div ref={ref} className="relative w-48">
            <div className="flex items-center bg-surface-base border border-stroke-divider rounded-lg focus-within:ring-1 focus-within:ring-accent-default overflow-hidden">
                <input
                    type="text"
                    value={displayLabel ?? value}
                    readOnly
                    onFocus={() => setOpen(true)}
                    placeholder={displayLabel ?? inputPlaceholder}
                    className="flex-1 cursor-default bg-transparent px-3 py-1.5 text-fs-sm text-text-primary placeholder:text-text-tertiary focus:outline-none min-w-0"
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
                    {summaryPresets.map(p => (
                        <button
                            key={p.value}
                            type="button"
                            onMouseDown={e => { e.preventDefault(); onChange(p.value); setOpen(false); }}
                            className={`w-full text-left px-3 py-2 text-fs-sm transition-colors hover:bg-surface-subtle ${value === p.value ? 'text-accent-default bg-accent-default/5' : 'text-text-primary'}`}
                        >
                            {p.label}
                            {value === p.value && <span className="float-right text-accent-default">v</span>}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
};


export const SettingsPage: React.FC = () => {
    const t = useT();
    const [activeTab, setActiveTab] = useState<SettingsTab>('general');
    const [settings, setSettings] = useState<AllSettings | null>(null);
    const [originalSettings, setOriginalSettings] = useState<AllSettings | null>(null);
    const [saving, setSaving] = useState(false);
    const [appVersion, setAppVersion] = useState(APP_VERSION_FALLBACK);

    const [rebuildTagsProgress, setRebuildTagsProgress] = useState<{ current: number; total: number } | null>(null);
    const rebuildTagsUnlistenRef = useRef<(() => void) | null>(null);

    const [editingProfile, setEditingProfile] = useState<ProviderProfileData | null>(null);
    const [showApiKeys, setShowApiKeys] = useState<Record<string, boolean>>({});
    const [bilibiliLoggingIn, setBilibiliLoggingIn] = useState(false);
    const [whisperStatus, setWhisperStatus] = useState<Record<string, { downloaded: boolean }>>({});
    const [deletingWhisperModel, setDeletingWhisperModel] = useState<string | null>(null);

    const [exportModal, setExportModal] = useState<{
        open: boolean;
        step: 'confirm' | 'mnemonic';
        mnemonic: string;
        confirmed: boolean;
        destPath: string;
        loading: boolean;
    }>({ open: false, step: 'confirm', mnemonic: '', confirmed: false, destPath: '', loading: false });

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

    const [passwordData, setPasswordData] = useState({ oldPassword: '', newPassword: '', confirmPassword: '' });
    const [personalLoading, setPersonalLoading] = useState(false);
    const [newRecoveryModal, setNewRecoveryModal] = useState<{ open: boolean; mnemonic: string; confirmed: boolean }>({ open: false, mnemonic: '', confirmed: false });
    const [regenerateRecoveryModal, setRegenerateRecoveryModal] = useState<{ open: boolean; password: string; loading: boolean }>({ open: false, password: '', loading: false });
    const [showPasswords, setShowPasswords] = useState<Record<string, boolean>>({});
    const [verifyModal, setVerifyModal] = useState<{
        open: boolean;
        password: string;
        onVerified: () => void;
        loading: boolean;
        error: string;
        title: string;
    }>({ open: false, password: '', onVerified: () => { }, loading: false, error: '', title: '' });
    const [draggingAiActionId, setDraggingAiActionId] = useState<string | null>(null);
    const [dragOverAiActionId, setDragOverAiActionId] = useState<string | null>(null);
    const aiActionDragRef = useRef<{
        isMouseDown: boolean;
        isDragging: boolean;
        startX: number;
        startY: number;
        draggedId: string | null;
        overId: string | null;
    }>({
        isMouseDown: false,
        isDragging: false,
        startX: 0,
        startY: 0,
        draggedId: null,
        overId: null,
    });


    const loadWhisperStatus = async () => {
        try {
            const status = await invoke<any>('whisper_model_status');
            setWhisperStatus(status);
        } catch (e) {
            console.error('Failed to load whisper status', e);
        }
    };

    const handleDeleteWhisperModel = async (modelName: string) => {
        if (!window.confirm(t('settings.whisper_delete_confirm', { model: modelName }))) {
            return;
        }

        setDeletingWhisperModel(modelName);
        try {
            const msg = await invoke<string>('whisper_delete_model', { modelName });
            toast.success(msg);
            await loadWhisperStatus();
        } catch (e: any) {
            toast.error(e.toString());
        } finally {
            setDeletingWhisperModel(null);
        }
    };

    useEffect(() => {
        loadSettings();
        loadWhisperStatus();
        getVersion()
            .then(setAppVersion)
            .catch(() => {});
    }, []);


    const loadSettings = async () => {
        try {
            const s = await invoke<AllSettings>('get_settings');
            const hydrated = {
                ...s,
                aiUsage: s.aiUsage ?? { mode: 'balanced' as const },
                editor: {
                    ...s.editor,
                    aiActions: normalizeEditorAiActions(s.editor?.aiActions),
                },
            };
            setSettings(hydrated);
            setOriginalSettings(hydrated);
        } catch (e) {
            console.error('Failed to load settings', e);
            toast.error(t('settings.load_failed'));
        }
    };

    const saveSettings = async (updated?: AllSettings) => {
        const target = updated ?? settings;
        if (!target) return;
        setSaving(true);
        try {
            await invoke('save_settings', { settings: target });
            setSettings(target);
            setOriginalSettings(target);
            toast.success(t('settings.saved'));
        } catch (e: any) {
            toast.error(t('settings.save_failed_with_reason', { error: e.toString() }));
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

    useEffect(() => {
        const resetAiActionDrag = () => {
            aiActionDragRef.current = {
                isMouseDown: false,
                isDragging: false,
                startX: 0,
                startY: 0,
                draggedId: null,
                overId: null,
            };
            setDraggingAiActionId(null);
            setDragOverAiActionId(null);
            document.body.style.userSelect = '';
            document.body.style.cursor = '';
        };

        const handleMouseMove = (event: MouseEvent) => {
            const drag = aiActionDragRef.current;
            if (!drag.isMouseDown || !drag.draggedId) return;

            if (!drag.isDragging) {
                const dx = event.clientX - drag.startX;
                const dy = event.clientY - drag.startY;
                if (Math.hypot(dx, dy) <= 8) return;

                drag.isDragging = true;
                setDraggingAiActionId(drag.draggedId);
                document.body.style.userSelect = 'none';
                document.body.style.cursor = 'grabbing';
            }

            event.preventDefault();
            const target = document.elementFromPoint(event.clientX, event.clientY) as HTMLElement | null;
            const row = target?.closest<HTMLElement>('[data-ai-action-id]');
            const overId = row?.dataset.aiActionId ?? null;
            drag.overId = overId && overId !== drag.draggedId ? overId : null;
            setDragOverAiActionId(drag.overId);
        };

        const handleMouseUp = () => {
            const drag = aiActionDragRef.current;
            const draggedId = drag.draggedId;
            const overId = drag.overId;
            const shouldReorder = drag.isDragging && draggedId && overId && draggedId !== overId;

            resetAiActionDrag();

            if (shouldReorder) {
                updateSettings(s => {
                    s.editor.aiActions = reorderEditorAiAction(s.editor.aiActions, draggedId, overId);
                });
            }
        };

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
            resetAiActionDrag();
        };
    }, [settings]);

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
        if (!window.confirm(t('settings.confirm_delete_provider'))) return;
        const profiles = settings.aiModels.providerProfiles.filter(p => p.id !== id);
        const updated = { ...settings, aiModels: { ...settings.aiModels, providerProfiles: profiles } };
        saveSettings(updated);
    };


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
                        <HotkeyInput value={settings.hotkeys.captureClipboard} onChange={v => updateSettings(s => { s.hotkeys.captureClipboard = v; })} className="w-48" />
                    </SettingRow>
                    <SettingRow label={t('settings.quick_input')} desc={t('settings.global_hotkey')}>
                        <HotkeyInput value={settings.hotkeys.quickInput} onChange={v => updateSettings(s => { s.hotkeys.quickInput = v; })} className="w-48" />
                    </SettingRow>
                </SectionCard>

                <SectionCard title={t('settings.reminders')}>
                    <SettingRow label={t('settings.reminders_ai_enabled')}>
                        <Toggle checked={settings.reminders?.aiEnabled ?? true} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { aiEnabled: true, enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.aiEnabled = v; })} />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_notifications_enabled')}>
                        <Toggle checked={settings.reminders?.enabled ?? true} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { aiEnabled: true, enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.enabled = v; })} />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_daily_time')}>
                        <TimeField value={settings.reminders?.dailyReminderTime ?? '09:00'} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { aiEnabled: true, enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.dailyReminderTime = v; })} className="w-32" />
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
                        <TimeField value={settings.reminders?.quietHoursStart ?? '22:00'} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { aiEnabled: true, enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.quietHoursStart = v; })} className="w-32" />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_quiet_end')}>
                        <TimeField value={settings.reminders?.quietHoursEnd ?? '08:00'} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { aiEnabled: true, enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.quietHoursEnd = v; })} className="w-32" />
                    </SettingRow>
                    <SettingRow label={t('settings.reminders_weekend_quiet')}>
                        <Toggle checked={settings.reminders?.weekendQuiet ?? false} onChange={v => updateSettings(s => { if (!s.reminders) s.reminders = { aiEnabled: true, enabled: true, dailyReminderTime: '09:00', quietHoursStart: '22:00', quietHoursEnd: '08:00', weekendQuiet: false }; s.reminders.weekendQuiet = v; })} />
                    </SettingRow>
                </SectionCard>


            </div>
        );
    };


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
                                                className={`relative flex flex-col items-start gap-1 rounded-xl p-3 border text-left transition-all ${isSelected
                                                    ? 'border-accent-default bg-accent-default/8 shadow-sm'
                                                    : 'border-stroke-divider hover:border-accent-default/40 hover:bg-surface-subtle'
                                                    }`}
                                            >
                                                <div className={`text-fs-sm font-semibold pr-5 ${isSelected ? 'text-accent-default' : 'text-text-primary'}`}>{card.label}</div>
                                                <div className="text-fs-xs text-text-tertiary leading-snug">{card.desc}</div>
                                                {card.local && (
                                                    <span className="text-fs-xs text-green-500 bg-green-500/10 px-1.5 py-0.5 rounded-full">{t('settings.local_provider')}</span>
                                                )}
                                            </button>
                                        );
                                    })}
                                </div>
                            </div>

                            <div>
                                <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_name')}</label>
                                <InputField value={editingProfile.name} onChange={v => setEditingProfile({ ...editingProfile, name: v })} placeholder={t('settings.provider_name_placeholder')} className="w-full" />
                            </div>

                            {(editingProfile.provider === 'ollama' || editingProfile.baseUrl) && (
                                <div>
                                    <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_base_url')}</label>
                                    <InputField value={editingProfile.baseUrl ?? ''} onChange={v => setEditingProfile({ ...editingProfile, baseUrl: v })} placeholder={t('settings.provider_base_url_placeholder')} className="w-full" />
                                </div>
                            )}

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
                                                    {t('settings.get_api_key')}
                                                </a>
                                            ) : null;
                                        })()}
                                    </div>
                                    <div className="relative">
                                        <InputField
                                            value={editingProfile.apiKey ?? ''}
                                            onChange={v => setEditingProfile({ ...editingProfile, apiKey: v })}
                                            placeholder={t('settings.api_key_placeholder')}
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

                <SectionCard title={t('settings.ai_usage_title')} desc={t('settings.ai_usage_desc')}>
                    <SettingRow label={t('settings.ai_usage_mode')} desc={t('settings.ai_usage_mode_desc')}>
                        <SelectField
                            value={settings.aiUsage?.mode ?? 'balanced'}
                            onChange={v => updateSettings(s => {
                                s.aiUsage = { mode: v as 'economy' | 'balanced' | 'quality' };
                            })}
                            options={[
                                { value: 'economy', label: t('settings.ai_usage_economy') },
                                { value: 'balanced', label: t('settings.ai_usage_balanced') },
                                { value: 'quality', label: t('settings.ai_usage_quality') },
                            ]}
                            className="w-44"
                        />
                    </SettingRow>
                    <div className="text-fs-xs text-text-tertiary leading-relaxed bg-surface-base border border-stroke-divider rounded-lg px-4 py-3">
                        {t('settings.ai_usage_note')}
                    </div>
                </SectionCard>

                <SectionCard title={t('settings.model_config_title')} desc={t('settings.model_config_desc')}>
                    {renderModelField(t('settings.model_chat'), t('settings.model_chat_desc'), ai.chatLlm, m => updateSettings(s => { s.aiModels.chatLlm = m; }), false, false, 'chat')}
                    {renderModelField(t('settings.model_processor'), t('settings.model_processor_desc'), ai.contentProcessorLlm, m => updateSettings(s => { s.aiModels.contentProcessorLlm = m; }), false, false, 'chat')}
                    {renderModelField(t('settings.model_vision'), t('settings.model_vision_desc'), ai.visionModel, m => updateSettings(s => { s.aiModels.visionModel = m; }), false, false, 'chat')}
                    {renderModelField(t('settings.model_embedding'), t('settings.model_embedding_desc'), ai.embeddingModel, m => updateSettings(s => { s.aiModels.embeddingModel = m; }), true, false, 'embedding')}
                    {renderModelField(t('settings.model_speech_to_text'), t('settings.model_speech_to_text_desc'), ai.speechToTextModel, m => updateSettings(s => { s.aiModels.speechToTextModel = m; }), true, true, 'speech-to-text')}
                </SectionCard>

                <SectionCard title={t('settings.model_summary_section')}>
                    <SettingRow label={t('settings.model_summary')} desc={t('settings.model_summary_desc')}>
                        <SummaryModelField
                            value={ai.summaryModel ?? 'follow_chat'}
                            onChange={v => updateSettings(s => { s.aiModels.summaryModel = v; })}
                            followChatLabel={t('settings.summary_follow_chat')}
                            followProcessorLabel={t('settings.summary_follow_processor')}
                            inputPlaceholder={t('settings.model_select_or_enter')}
                        />
                    </SettingRow>
                </SectionCard>


            </div>
        );
    };



    const handleTestModel = async (model: ModelSettings, category: string = 'chat') => {
        if (!model.provider || !model.model) {
            toast.error(t('settings.model_test_select_first'));
            return;
        }

        const tid = toast.loading(t('settings.model_testing', { model: model.model }));

        if (model.provider === 'local' && (category === 'embedding' || category === 'speech-to-text')) {
            // Local models handled internally
            const responseLabel = category === 'embedding' ? 'Local Embedder Ready' : 'Whisper Model Ready';
            setTimeout(() => {
                toast.success(t('settings.model_test_success', { response: responseLabel }), { id: tid });
            }, 500);
            return;
        }

        let baseUrl = model.baseUrl;
        let apiKey = model.apiKey;

        const profile = settings?.aiModels.providerProfiles.find(p => {
            if (p.provider !== model.provider) return false;
            if (model.baseUrl) return p.baseUrl === model.baseUrl;
            return true;
        });
        if ((!baseUrl || !apiKey) && profile) {
            baseUrl = profile.baseUrl;
            apiKey = profile.apiKey ?? apiKey;
        } else if (model.provider === 'ollama') {
            baseUrl = baseUrl || 'http://localhost:11434';
        }

        try {
            const res = await invoke<string>('test_model_connection', {
                provider: model.provider,
                model: model.model,
                baseUrl: baseUrl,
                apiKey: apiKey
            });
            toast.success(t('settings.model_test_success', { response: res }), { id: tid });
        } catch (e: any) {
            toast.error(t('settings.model_test_failed', { error: e.toString() }), { id: tid });
        }
    };

    const renderModelField = (label: string, desc: string, model: ModelSettings, onChange: (m: ModelSettings) => void, includeLocal?: boolean, isSpeechToText?: boolean, category: 'embedding' | 'speech-to-text' | 'chat' = 'chat') => {
        const profileOptions = settings?.aiModels.providerProfiles ?? [];
        const providerOptions = buildProviderOptions(profileOptions, PROVIDER_CARDS, !!includeLocal, t('settings.system_default_local_provider'));
        return (
            <div className="bg-surface-base rounded-lg px-4 py-3 border border-stroke-divider space-y-2">
                <div className="text-fs-sm font-medium text-text-primary">{label}</div>
                <div className="text-fs-xs text-text-tertiary">{desc}</div>
                <div className="grid grid-cols-2 gap-3 pt-1">
                    <div>
                        <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.provider_type')}</label>
                        <SelectField
                            value={getSelectedProviderOptionValue(model, profileOptions)}
                            onChange={v => {
                                onChange(applyProviderOptionSelection(v, model, profileOptions, PROVIDER_CARDS, category, POPULAR_MODELS));
                            }}
                            options={providerOptions}
                            className="w-full"
                        />
                    </div>
                    <div>
                        <label className="text-fs-xs text-text-secondary mb-1 block">{t('settings.model_name')}</label>
                        <div className="flex gap-2 items-start">
                            <ModelComboField
                                value={model.model}
                                onChange={v => onChange({ ...model, model: v })}
                                provider={model.provider}
                                category={category}
                                inputPlaceholder={t('settings.model_select_or_enter')}
                                className="flex-1"
                            />
                            {isSpeechToText && model.provider === 'local' && !whisperStatus[model.model]?.downloaded ? (
                                <button
                                    onClick={async () => {
                                        const tid = toast.loading(t('settings.whisper_downloading', { model: model.model }));
                                        try {
                                            const msg = await invoke<string>('whisper_download_model', { modelName: model.model });
                                            toast.success(msg, { id: tid });
                                            loadWhisperStatus();
                                        } catch (e: any) {
                                            toast.error(e.toString(), { id: tid });
                                        }
                                    }}
                                    className="px-3 py-1.5 mt-0.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-sm hover:text-accent-default hover:border-accent-default/30 transition-colors shrink-0"
                                >
                                    {t('settings.whisper_download')}
                                </button>
                            ) : isSpeechToText && model.provider === 'local' ? (
                                <button
                                    onClick={() => handleDeleteWhisperModel(model.model)}
                                    disabled={deletingWhisperModel === model.model}
                                    className="px-3 py-1.5 mt-0.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-sm hover:text-color-danger hover:border-color-danger disabled:opacity-50 disabled:cursor-not-allowed transition-colors shrink-0 inline-flex items-center gap-1.5"
                                    title={t('settings.whisper_delete')}
                                >
                                    <Trash2 className="w-3.5 h-3.5" />
                                    {deletingWhisperModel === model.model ? t('settings.whisper_deleting') : t('settings.whisper_delete')}
                                </button>
                            ) : (
                                <button
                                    onClick={() => handleTestModel(model, category)}
                                    className="px-3 py-1.5 mt-0.5 bg-surface-subtle border border-stroke-divider text-text-secondary rounded-lg text-fs-sm hover:text-accent-default hover:border-accent-default/30 transition-colors shrink-0"
                                >
                                    {t('settings.model_test_btn')}
                                </button>
                            )}
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

                <SectionCard
                    title={t('settings.telegram_title')}
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
                            <SettingRow label={t('settings.telegram_bot_token_label')} desc={t('settings.telegram_token_desc')}>
                                <div className="flex items-center gap-2">
                                    <InputField
                                        value={settings.telegram.botToken}
                                        onChange={v => updateSettings(s => { s.telegram.botToken = v; })}
                                        type={showPasswords.botToken ? 'text' : 'password'}
                                        className="w-72"
                                        placeholder={t('settings.telegram_token_placeholder')}
                                    />
                                    <button
                                        onClick={() => setShowPasswords(prev => ({ ...prev, botToken: !prev.botToken }))}
                                        className="p-1.5 text-text-tertiary hover:text-text-secondary transition-colors"
                                    >
                                        {showPasswords.botToken ? <EyeOff size={14} /> : <Eye size={14} />}
                                    </button>
                                </div>
                            </SettingRow>
                            <SettingRow label={t('settings.telegram_allowed_user_ids_label')} desc={t('settings.telegram_userids_desc')}>
                                <div className="flex items-center gap-2">
                                    <InputField
                                        value={settings.telegram.allowedUserIds.join(', ')}
                                        onChange={v => updateSettings(s => {
                                            s.telegram.allowedUserIds = v.split(',').map(id => parseInt(id.trim())).filter(id => !isNaN(id));
                                        })}
                                        className="w-72"
                                        placeholder={t('settings.telegram_user_ids_placeholder')}
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
                                                    userIds: settings.telegram.allowedUserIds,
                                                    language: settings.general.language
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


            </div>
        );
    };


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
                        <InputField value={settings.knowledge.kbPath} onChange={v => updateSettings(s => { s.knowledge.kbPath = v; })} placeholder={t('settings.knowledge_path_placeholder')} className="w-64" />
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



            </div>
        );
    };


    const renderOther = () => {
        if (!settings) return null;
        const aiActions = normalizeEditorAiActions(settings.editor.aiActions);
        const enabledAiActions = getEnabledEditorAiActions(aiActions);
        const displayAiActionLabel = (action: EditorAiAction) => (
            action.labelKey.includes('.') ? t(action.labelKey as any) : action.labelKey
        );
        const setAiActions = (actions: EditorAiAction[]) => {
            updateSettings(s => { s.editor.aiActions = actions; });
        };
        const patchAiAction = (id: string, patch: Partial<EditorAiAction>) => {
            setAiActions(updateEditorAiAction(aiActions, id, patch));
        };
        const addAiAction = () => {
            setAiActions([createEditorAiAction('custom'), ...aiActions]);
        };

        return (
            <div className="max-w-5xl mx-auto">
                <div className="mb-7">
                    <h3 className="text-fs-2xl font-bold text-text-primary">{t('settings.other_section_title')}</h3>
                    <p className="text-fs-sm text-text-tertiary mt-1">{t('settings.other_section_desc')}</p>
                </div>

                <SectionCard title={t('settings.editor_override_title')} desc={t('settings.editor_override_desc')}>
                    <textarea
                        value={settings.editor.promptInstructionOverride || ''}
                        onChange={e => updateSettings(s => { s.editor.promptInstructionOverride = e.target.value; })}
                        placeholder={t('settings.ai_prompt_instruction_placeholder')}
                        rows={4}
                        className="w-full bg-surface-base border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-y min-h-[108px] focus:outline-none focus:ring-1 focus:ring-accent-default"
                    />
                </SectionCard>

                <SectionCard
                    title={t('settings.ai_optimize_actions_title')}
                    desc={t('settings.ai_optimize_actions_desc')}
                    action={
                        <div className="flex items-center gap-2">
                            <button
                                type="button"
                                onClick={() => setAiActions(cloneDefaultEditorAiActions())}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-stroke-divider text-fs-xs text-text-secondary hover:bg-surface-subtle hover:text-text-primary transition-colors"
                            >
                                <RefreshCw className="w-3.5 h-3.5" />
                                {t('settings.ai_action_reset')}
                            </button>
                            <button
                                type="button"
                                onClick={addAiAction}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-accent-default text-white text-fs-xs hover:bg-accent-light1 transition-colors"
                            >
                                <Plus className="w-3.5 h-3.5" />
                                {t('settings.ai_action_add')}
                            </button>
                        </div>
                    }
                >
                    <div className="grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_260px] gap-4">
                        <div className="space-y-3">
                        {aiActions.map((action, index) => (
                            <div
                                key={action.id}
                                data-ai-action-id={action.id}
                                className={`bg-surface-base border rounded-lg p-4 transition-colors ${draggingAiActionId === action.id
                                    ? 'opacity-50 border-accent-default'
                                    : dragOverAiActionId === action.id
                                        ? 'border-accent-default bg-accent-default/5'
                                        : 'border-stroke-divider'
                                    }`}
                            >
                                <div className="grid grid-cols-1 lg:grid-cols-[2rem_minmax(10rem,14rem)_1fr_auto] gap-3 items-start">
                                    <div
                                        onMouseDown={event => {
                                            if (event.button !== 0) return;
                                            event.preventDefault();
                                            aiActionDragRef.current = {
                                                isMouseDown: true,
                                                isDragging: false,
                                                startX: event.clientX,
                                                startY: event.clientY,
                                                draggedId: action.id,
                                                overId: null,
                                            };
                                        }}
                                        onDragStart={event => event.preventDefault()}
                                        className="hidden lg:block pt-2 text-text-tertiary cursor-grab active:cursor-grabbing hover:text-text-secondary"
                                        title={t('settings.ai_action_drag')}
                                    >
                                        <GripVertical className="w-4 h-4" />
                                    </div>
                                    <InputField
                                        value={displayAiActionLabel(action)}
                                        onChange={value => patchAiAction(action.id, { labelKey: value })}
                                        placeholder={t('settings.ai_action_name_placeholder')}
                                        className="w-full"
                                    />
                                    <div className="flex items-center justify-end gap-2">
                                        <Toggle checked={action.enabled} onChange={value => patchAiAction(action.id, { enabled: value })} />
                                        <button
                                            type="button"
                                            onClick={() => setAiActions(moveEditorAiAction(aiActions, action.id, 'up'))}
                                            disabled={index === 0}
                                            className="w-8 h-8 rounded-lg border border-stroke-divider text-text-secondary hover:bg-surface-subtle disabled:opacity-40 disabled:hover:bg-transparent"
                                            title={t('settings.ai_action_move_up')}
                                        >
                                            <ArrowUp className="w-4 h-4 mx-auto" />
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => setAiActions(moveEditorAiAction(aiActions, action.id, 'down'))}
                                            disabled={index === aiActions.length - 1}
                                            className="w-8 h-8 rounded-lg border border-stroke-divider text-text-secondary hover:bg-surface-subtle disabled:opacity-40 disabled:hover:bg-transparent"
                                            title={t('settings.ai_action_move_down')}
                                        >
                                            <ArrowDown className="w-4 h-4 mx-auto" />
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => setAiActions(removeEditorAiAction(aiActions, action.id))}
                                            className="w-8 h-8 inline-flex items-center justify-center rounded-lg border border-red-500/20 text-red-500 hover:bg-red-500/5 transition-colors"
                                            title={t('common.delete')}
                                        >
                                            <Trash2 className="w-4 h-4" />
                                        </button>
                                    </div>
                                </div>
                                <textarea
                                    value={action.prompt}
                                    onChange={event => patchAiAction(action.id, { prompt: event.target.value })}
                                    placeholder={t('settings.ai_action_prompt_placeholder')}
                                    rows={3}
                                    className="w-full mt-3 bg-surface-layer border border-stroke-divider rounded-lg px-3 py-2 text-fs-sm text-text-primary placeholder:text-text-tertiary resize-y min-h-[76px] focus:outline-none focus:ring-1 focus:ring-accent-default"
                                />
                            </div>
                        ))}
                        </div>
                        <div className="rounded-lg border border-stroke-divider bg-surface-base p-4 h-fit">
                            <div className="text-fs-sm font-semibold text-text-primary">{t('settings.ai_action_preview_title')}</div>
                            <p className="text-fs-xs text-text-tertiary mt-1">{t('settings.ai_action_menu_note')}</p>
                            <div className="mt-4 rounded-lg border border-stroke-divider bg-surface-flyout p-1 shadow-lg">
                                {enabledAiActions.map(action => (
                                    <div key={action.id} className="flex items-center gap-2 px-2.5 py-2 rounded-md text-fs-xs text-text-secondary">
                                        <Sparkles className="w-3.5 h-3.5 text-accent-default" />
                                        <span className="truncate">{displayAiActionLabel(action)}</span>
                                    </div>
                                ))}
                                <div className="h-px bg-stroke-divider my-1 mx-1" />
                                <div className="flex items-center gap-2 px-2.5 py-2 rounded-md text-fs-xs text-text-secondary">
                                    <Sparkles className="w-3.5 h-3.5 text-accent-default" />
                                    <span>{t('editor.ai_custom')}</span>
                                </div>
                            </div>
                        </div>
                    </div>
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
                    <SettingRow label={t('settings.export_subdir')}>
                        <InputField value={settings.editor.exportSubdir} onChange={v => updateSettings(s => { s.editor.exportSubdir = v; })} className="w-52" />
                    </SettingRow>
                </SectionCard>


            </div>
        );
    };


    const handlePasswordChange = async () => {
        if (!settings) return;
        if (!passwordData.oldPassword || !passwordData.newPassword) {
            toast.error(t('settings.password_required'));
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
        const tid = toast.loading(t('settings.password_changing'));
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
        const password = regenerateRecoveryModal.password;
        if (!password) return;

        setRegenerateRecoveryModal(s => ({ ...s, loading: true }));
        setPersonalLoading(true);
        const tid = toast.loading(t('settings.recovery_generating'));
        try {
            const mnemonic = await invoke<string>('reset_recovery_phrase', {
                payload: {
                    password,
                    kbPath: settings.knowledge.kbPath
                }
            });
            toast.success(t('settings.regenerate_success'), { id: tid });
            setRegenerateRecoveryModal({ open: false, password: '', loading: false });
            setNewRecoveryModal({ open: true, mnemonic, confirmed: false });
        } catch (e: any) {
            toast.error(`${t('common.error')}: ${e.toString()}`, { id: tid });
            setRegenerateRecoveryModal(s => ({ ...s, loading: false }));
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
                                onClick={() => setRegenerateRecoveryModal({ open: true, password: '', loading: false })}
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


    return (
        <div className="flex h-full w-full bg-surface-base">
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

                <div className="mt-auto pt-6 px-3 border-t border-stroke-divider/30">
                    <div className="flex flex-col gap-1">
                        <span className="text-[10px] font-bold text-accent-default tracking-widest uppercase opacity-70">
                            InsightCAP v{appVersion}
                        </span>

                        <span className="text-[9px] text-text-tertiary/40 leading-tight mt-1 whitespace-pre-line">
                            {t('common.copyright')}
                        </span>
                    </div>
                </div>
            </div>

            <div className="flex-1 p-10 pb-32 overflow-y-auto bg-surface-base/50 relative">
                {activeTab === 'general' && renderGeneral()}
                {activeTab === 'personal' && renderPersonal()}
                {activeTab === 'provider' && renderProvider()}
                {activeTab === 'ai' && renderAI()}
                {activeTab === 'knowledge' && renderKnowledge()}

                {activeTab === 'other' && renderOther()}

                {settings && originalSettings && JSON.stringify(settings) !== JSON.stringify(originalSettings) && (
                    <div className="fixed bottom-10 right-10 z-[100] animate-in slide-in-from-bottom-4 duration-300">
                        <button
                            onClick={() => saveSettings()}
                            disabled={saving}
                            className="flex items-center gap-2.5 bg-accent-default text-white px-8 py-3.5 rounded-full shadow-2xl hover:bg-accent-light1 transition-all hover:scale-105 active:scale-95 disabled:opacity-50 border border-white/10"
                        >
                            {saving ? <RefreshCw className="w-5 h-5 animate-spin" /> : <Save className="w-5 h-5" />}
                            <span className="font-bold tracking-wide">
                                {saving ? t('common.saving') : t('common.save')}
                            </span>
                        </button>
                    </div>
                )}
            </div>

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
                                    } catch {
                                        setVerifyModal(s => ({ ...s, loading: false, error: t('auth.login.invalid_credentials') }));
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

            {regenerateRecoveryModal.open && (
                <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm">
                    <div className="bg-surface-base border border-stroke-divider rounded-2xl p-8 w-[440px] max-w-[calc(100vw-2rem)] shadow-2xl animate-in fade-in zoom-in duration-200">
                        <div className="flex items-center gap-3 mb-6">
                            <div className="p-2 bg-accent-default/10 rounded-full">
                                <ShieldCheck className="w-6 h-6 text-accent-default" />
                            </div>
                            <h3 className="text-fs-xl font-bold text-text-primary">{t('settings.regenerate_recovery_code')}</h3>
                        </div>

                        <p className="text-fs-sm text-text-secondary mb-5 leading-relaxed">
                            {t('settings.regenerate_confirm')}
                        </p>

                        <div className="space-y-1.5 mb-8">
                            <label className="text-fs-xs text-text-secondary font-medium">{t('auth.login.password_label')}</label>
                            <div className="relative">
                                <input
                                    autoFocus
                                    type={showPasswords.recoveryRegenerate ? 'text' : 'password'}
                                    value={regenerateRecoveryModal.password}
                                    onChange={e => setRegenerateRecoveryModal(s => ({ ...s, password: e.target.value }))}
                                    onKeyDown={e => {
                                        if (e.key === 'Enter' && !regenerateRecoveryModal.loading && regenerateRecoveryModal.password) {
                                            handleRegenerateRecovery();
                                        }
                                    }}
                                    className="w-full bg-surface-layer border border-stroke-divider rounded-xl px-4 py-2.5 pr-10 text-fs-sm focus:outline-none focus:ring-2 focus:ring-accent-default/20 focus:border-accent-default transition-all"
                                    placeholder={t('auth.login.password_placeholder')}
                                />
                                <button
                                    type="button"
                                    onClick={() => setShowPasswords(p => ({ ...p, recoveryRegenerate: !p.recoveryRegenerate }))}
                                    className="absolute right-3 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary transition-colors"
                                >
                                    {showPasswords.recoveryRegenerate ? <EyeOff size={16} /> : <Eye size={16} />}
                                </button>
                            </div>
                        </div>

                        <div className="flex gap-3 justify-end">
                            <button
                                onClick={() => setRegenerateRecoveryModal({ open: false, password: '', loading: false })}
                                disabled={regenerateRecoveryModal.loading}
                                className="px-5 py-2.5 text-fs-sm font-medium text-text-secondary hover:text-text-primary hover:bg-surface-subtle rounded-xl transition-all disabled:opacity-50"
                            >
                                {t('common.cancel')}
                            </button>
                            <button
                                onClick={handleRegenerateRecovery}
                                disabled={regenerateRecoveryModal.loading || !regenerateRecoveryModal.password}
                                className="px-8 py-2.5 bg-accent-default text-white rounded-xl font-semibold hover:bg-accent-light1 transition-all shadow-lg shadow-accent-default/20 disabled:opacity-50 active:scale-95 flex items-center gap-2"
                            >
                                {regenerateRecoveryModal.loading ? (
                                    <>
                                        <RefreshCw className="w-4 h-4 animate-spin" />
                                        {t('settings.recovery_generating')}
                                    </>
                                ) : (
                                    t('common.confirm')
                                )}
                            </button>
                        </div>
                    </div>
                </div>
            )}

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
                                        window.location.reload(); // Reload to ensure security state is refreshed
                                    } catch (e) {
                                        toast.error(t('settings.confirmation_failed'));
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
