/**
 * 手機端設定持久化（AsyncStorage）
 *
 * 三種推理模式設定 + 桌面連線資訊
 */

import AsyncStorage from '@react-native-async-storage/async-storage';
import type { InferenceMode } from './inference/index';

// ─── Types ──────────────────────────────────────────────────────────────────

export interface LocalSettings {
  modelPath: string;
  modelVariant: 'gemma4-1b' | 'gemma4-4b';
  maxTokens: number;
  hfToken: string;
}

export interface CloudSettings {
  provider: 'claude' | 'openai' | 'gemini';
  apiKey: string;
  model: string;
}

export interface DesktopSettings {
  url: string;
  token: string;
}

export interface MobileSettings {
  inferenceMode: InferenceMode;
  local: LocalSettings;
  cloud: CloudSettings;
  desktop: DesktopSettings;
}

// ─── Defaults ───────────────────────────────────────────────────────────────

const DEFAULTS: MobileSettings = {
  inferenceMode: 'desktop',
  local: {
    modelPath: '',
    modelVariant: 'gemma4-1b',
    maxTokens: 2048,
    hfToken: '',
  },
  cloud: {
    provider: 'claude',
    apiKey: '',
    model: 'claude-sonnet-4-6',
  },
  desktop: {
    url: '',
    token: '',
  },
};

// ─── In-memory Cache ────────────────────────────────────────────────────────

let _cache: MobileSettings = { ...DEFAULTS };

const STORAGE_KEY = '@insightcap/settings';

// ─── Load / Save ────────────────────────────────────────────────────────────

export async function loadSettings(): Promise<MobileSettings> {
  try {
    const raw = await AsyncStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      _cache = {
        ...DEFAULTS,
        ...parsed,
        local: { ...DEFAULTS.local, ...parsed.local },
        cloud: { ...DEFAULTS.cloud, ...parsed.cloud },
        desktop: { ...DEFAULTS.desktop, ...parsed.desktop },
      };
    }
  } catch {
    // ignore
  }
  return _cache;
}

export async function saveSettings(
  settings: Partial<MobileSettings>,
): Promise<void> {
  _cache = { ..._cache, ...settings };
  await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(_cache));
}

// ─── Sync Getters（供 inference providers 使用）─────────────────────────────

export function getSettings(): MobileSettings {
  return _cache;
}

export function getLocalSettings(): LocalSettings {
  return _cache.local;
}

export function getCloudSettings(): CloudSettings {
  return _cache.cloud;
}

export function getDesktopSettings(): DesktopSettings {
  return _cache.desktop;
}

export function getInferenceMode(): InferenceMode {
  return _cache.inferenceMode;
}
