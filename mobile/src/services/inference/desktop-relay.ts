/**
 * Mode C — Desktop Relay Provider
 *
 * 將 message 發送到桌面端 Axum /api/chat
 * 桌面完成 RAG + LLM，以 SSE stream 回傳結果
 * 手機不需要 API key，完全複用桌面設定
 */

import type { InferenceProvider, GenerateOptions } from './index';
import { getDesktopSettings } from '../settings-store';

export class DesktopRelayProvider implements InferenceProvider {
  private abortController: AbortController | null = null;

  async isAvailable(): Promise<boolean> {
    const cfg = getDesktopSettings();
    if (!cfg?.url || !cfg?.token) return false;
    try {
      const res = await fetch(`${cfg.url}/api/health`, {
        signal: AbortSignal.timeout(3000),
      });
      return res.ok;
    } catch {
      return false;
    }
  }

  async generate(
    message: string,
    options: GenerateOptions,
  ): Promise<void> {
    const cfg = getDesktopSettings();
    if (!cfg?.url || !cfg?.token) {
      options.onError('請先在設定中填入桌面端連線資訊');
      return;
    }

    this.abortController = new AbortController();

    const history = (options.history ?? []).map(h => [h.role, h.content]);

    try {
      const res = await fetch(`${cfg.url}/api/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${cfg.token}`,
        },
        body: JSON.stringify({ message, history }),
        signal: this.abortController.signal,
      });

      if (res.status === 401) {
        options.onError('Token 無效，請重新設定');
        return;
      }
      if (res.status === 503) {
        options.onError('桌面端 LLM 未設定');
        return;
      }
      if (!res.ok) {
        options.onError(`連線錯誤 ${res.status}`);
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
          if (!line.startsWith('data: ')) continue;
          const token = line.slice(6);
          if (token) {
            fullText += token;
            options.onToken(token);
          }
        }
      }

      options.onDone(fullText);
    } catch (e: unknown) {
      if ((e as Error)?.name === 'AbortError') return;
      options.onError('無法連線到桌面端，請確認在同一 WiFi 網路');
    } finally {
      this.abortController = null;
    }
  }

  cancel(): void {
    this.abortController?.abort();
    this.abortController = null;
  }
}
