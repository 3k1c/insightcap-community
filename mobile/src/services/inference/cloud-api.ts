/**
 * Mode B — Cloud API Provider
 *
 * 手機直連雲端 LLM（Claude / OpenAI / Gemini）
 * 支援 SSE streaming
 */

import type { InferenceProvider, GenerateOptions } from './index';
import { getCloudSettings } from '../settings-store';

type CloudProvider = 'claude' | 'openai' | 'gemini';

interface CloudConfig {
  provider: CloudProvider;
  apiKey: string;
  model: string;
}

// ─── Endpoints ──────────────────────────────────────────────────────────────

const ENDPOINTS: Record<CloudProvider, string> = {
  claude: 'https://api.anthropic.com/v1/messages',
  openai: 'https://api.openai.com/v1/chat/completions',
  gemini: 'https://generativelanguage.googleapis.com/v1beta/models',
};

// ─── Request Builders ───────────────────────────────────────────────────────

function buildClaudeRequest(
  message: string,
  history: GenerateOptions['history'],
  config: CloudConfig,
  contextText?: string,
) {
  const systemPrompt = contextText
    ? `你是 InsightCAP 知識助理。請參考以下知識庫內容回答：\n\n${contextText}`
    : '你是 InsightCAP 知識助理。';

  return {
    url: ENDPOINTS.claude,
    headers: {
      'Content-Type': 'application/json',
      'x-api-key': config.apiKey,
      'anthropic-version': '2023-06-01',
    },
    body: {
      model: config.model || 'claude-sonnet-4-6',
      max_tokens: 2048,
      system: systemPrompt,
      messages: [
        ...(history ?? []).map(h => ({ role: h.role, content: h.content })),
        { role: 'user', content: message },
      ],
      stream: true,
    },
  };
}

function buildOpenAIRequest(
  message: string,
  history: GenerateOptions['history'],
  config: CloudConfig,
  contextText?: string,
) {
  const systemPrompt = contextText
    ? `你是 InsightCAP 知識助理。請參考以下知識庫內容回答：\n\n${contextText}`
    : '你是 InsightCAP 知識助理。';

  return {
    url: ENDPOINTS.openai,
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${config.apiKey}`,
    },
    body: {
      model: config.model || 'gpt-4o',
      messages: [
        { role: 'system', content: systemPrompt },
        ...(history ?? []).map(h => ({ role: h.role, content: h.content })),
        { role: 'user', content: message },
      ],
      stream: true,
    },
  };
}

function buildGeminiRequest(
  message: string,
  history: GenerateOptions['history'],
  config: CloudConfig,
  contextText?: string,
) {
  const systemInstruction = contextText
    ? `你是 InsightCAP 知識助理。請參考以下知識庫內容回答：\n\n${contextText}`
    : '你是 InsightCAP 知識助理。';

  const model = config.model || 'gemini-2.5-flash';
  return {
    url: `${ENDPOINTS.gemini}/${model}:streamGenerateContent?alt=sse&key=${config.apiKey}`,
    headers: { 'Content-Type': 'application/json' },
    body: {
      system_instruction: { parts: [{ text: systemInstruction }] },
      contents: [
        ...(history ?? []).map(h => ({
          role: h.role === 'assistant' ? 'model' : 'user',
          parts: [{ text: h.content }],
        })),
        { role: 'user', parts: [{ text: message }] },
      ],
      generationConfig: { maxOutputTokens: 2048 },
    },
  };
}

// ─── SSE Parsers ────────────────────────────────────────────────────────────

function parseClaudeSSE(line: string): string | null {
  if (!line.startsWith('data: ')) return null;
  try {
    const data = JSON.parse(line.slice(6));
    return data.delta?.text ?? null;
  } catch {
    return null;
  }
}

function parseOpenAISSE(line: string): string | null {
  if (!line.startsWith('data: ') || line === 'data: [DONE]') return null;
  try {
    const data = JSON.parse(line.slice(6));
    return data.choices?.[0]?.delta?.content ?? null;
  } catch {
    return null;
  }
}

function parseGeminiSSE(line: string): string | null {
  if (!line.startsWith('data: ')) return null;
  try {
    const data = JSON.parse(line.slice(6));
    return data.candidates?.[0]?.content?.parts?.[0]?.text ?? null;
  } catch {
    return null;
  }
}

// ─── Provider ───────────────────────────────────────────────────────────────

export class CloudApiProvider implements InferenceProvider {
  private abortController: AbortController | null = null;

  async isAvailable(): Promise<boolean> {
    const cfg = getCloudSettings();
    return !!cfg?.apiKey?.trim();
  }

  async generate(
    message: string,
    options: GenerateOptions,
  ): Promise<void> {
    const cfg = getCloudSettings();
    if (!cfg?.apiKey) {
      options.onError('請先在設定中輸入雲端 API Key');
      return;
    }

    // 嘗試取得桌面 RAG context
    let contextText: string | undefined;
    try {
      const { fetchRagContext } = await import('../desktop-api');
      const rag = await fetchRagContext(message);
      contextText = rag?.context_text;
    } catch {
      // 離線
    }

    this.abortController = new AbortController();

    let requestConfig: {
      url: string;
      headers: Record<string, string>;
      body: object;
    };
    let parseSSE: (line: string) => string | null;

    switch (cfg.provider) {
      case 'openai':
        requestConfig = buildOpenAIRequest(message, options.history, cfg, contextText);
        parseSSE = parseOpenAISSE;
        break;
      case 'gemini':
        requestConfig = buildGeminiRequest(message, options.history, cfg, contextText);
        parseSSE = parseGeminiSSE;
        break;
      default:
        requestConfig = buildClaudeRequest(message, options.history, cfg, contextText);
        parseSSE = parseClaudeSSE;
    }

    try {
      const res = await fetch(requestConfig.url, {
        method: 'POST',
        headers: requestConfig.headers,
        body: JSON.stringify(requestConfig.body),
        signal: this.abortController.signal,
      });

      if (!res.ok) {
        options.onError(`API 錯誤 ${res.status}`);
        return;
      }

      const reader = res.body?.getReader();
      const decoder = new TextDecoder();
      let fullText = '';
      let buffer = '';

      while (reader) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() ?? '';

        for (const line of lines) {
          const token = parseSSE(line.trim());
          if (token) {
            fullText += token;
            options.onToken(token);
          }
        }
      }

      options.onDone(fullText);
    } catch (e: unknown) {
      if ((e as Error)?.name === 'AbortError') return;
      options.onError((e as Error)?.message ?? '請求失敗');
    } finally {
      this.abortController = null;
    }
  }

  cancel(): void {
    this.abortController?.abort();
    this.abortController = null;
  }
}
