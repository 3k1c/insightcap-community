/**
 * 桌面端 HTTP API 客戶端
 *
 * 所有與桌面端 Axum (0.0.0.0:3030) 的通訊封裝
 * 包含：capture / rag / health / conversations / sources
 */

import { getDesktopSettings } from './settings-store';

// ─── Types ──────────────────────────────────────────────────────────────────

export interface ConversationItem {
  id: string;
  title: string;
  summary?: string;
  updated_at: string;
}

export interface MessageItem {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  created_at: string;
}

export interface SourceItem {
  id: string;
  title: string;
  media_type?: string;
  source_category: string;
  capture_count: number;
  captured_at: string;
}

export interface RagResult {
  context_text: string;
  context: unknown;
}

// ─── Base Fetch ─────────────────────────────────────────────────────────────

async function desktopFetch(
  path: string,
  options: RequestInit = {},
): Promise<Response> {
  const cfg = getDesktopSettings();
  if (!cfg?.url || !cfg?.token) {
    throw new Error('Desktop not configured');
  }

  return fetch(`${cfg.url}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${cfg.token}`,
      ...(options.headers as Record<string, string>),
    },
    signal: options.signal ?? AbortSignal.timeout(8000),
  });
}

// ─── Health ─────────────────────────────────────────────────────────────────

export async function checkDesktopHealth(): Promise<boolean> {
  try {
    const cfg = getDesktopSettings();
    if (!cfg?.url) return false;
    const res = await fetch(`${cfg.url}/api/health`, {
      signal: AbortSignal.timeout(3000),
    });
    return res.ok;
  } catch {
    return false;
  }
}

// ─── Capture ────────────────────────────────────────────────────────────────

export async function captureToDesktop(content: string): Promise<void> {
  const res = await desktopFetch('/api/capture', {
    method: 'POST',
    body: JSON.stringify({ content }),
  });
  if (!res.ok) throw new Error(`Capture failed: ${res.status}`);
}

// ─── RAG ────────────────────────────────────────────────────────────────────

export async function fetchRagContext(
  query: string,
  limit = 5,
): Promise<RagResult | null> {
  try {
    const res = await desktopFetch('/api/rag', {
      method: 'POST',
      body: JSON.stringify({ query, limit }),
      signal: AbortSignal.timeout(5000),
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

// ─── Conversations ──────────────────────────────────────────────────────────

export async function fetchConversations(): Promise<ConversationItem[]> {
  try {
    const res = await desktopFetch('/api/conversations', { method: 'GET' });
    if (!res.ok) return [];
    return res.json();
  } catch {
    return [];
  }
}

export async function fetchMessages(
  conversationId: string,
): Promise<MessageItem[]> {
  try {
    const res = await desktopFetch(
      `/api/conversations/${conversationId}/messages`,
      { method: 'GET' },
    );
    if (!res.ok) return [];
    return res.json();
  } catch {
    return [];
  }
}

export async function createConversation(
  title?: string,
): Promise<{ id: string; title: string } | null> {
  try {
    const res = await desktopFetch('/api/conversations', {
      method: 'POST',
      body: JSON.stringify({ title: title ?? '新對話' }),
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

// ─── Sources ────────────────────────────────────────────────────────────────

export async function fetchSources(): Promise<SourceItem[]> {
  try {
    const res = await desktopFetch('/api/sources', { method: 'GET' });
    if (!res.ok) return [];
    return res.json();
  } catch {
    return [];
  }
}
