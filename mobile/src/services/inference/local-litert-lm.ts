/**
 * Mode A — Local LiteRT-LM Provider
 *
 * 透過 LiteRTLMModule Kotlin bridge 呼叫 on-device Gemma 4
 * 推理完全在手機上，不需要網路
 */

import { NativeModules, NativeEventEmitter, Platform } from 'react-native';
import type { InferenceProvider, GenerateOptions } from './index';
import { getLocalSettings } from '../settings-store';

const { LiteRTLM } = NativeModules;
const emitter = LiteRTLM ? new NativeEventEmitter(LiteRTLM) : null;

// ─── Prompt Builder ─────────────────────────────────────────────────────────

function buildPrompt(
  message: string,
  history: GenerateOptions['history'],
  contextText?: string,
): string {
  const systemBlock = contextText
    ? `<start_of_turn>system\n你是 InsightCAP 知識助理。請參考以下知識庫內容回答：\n\n${contextText}<end_of_turn>\n`
    : `<start_of_turn>system\n你是 InsightCAP 知識助理。<end_of_turn>\n`;

  const historyBlock = (history ?? [])
    .map(
      ({ role, content }) =>
        `<start_of_turn>${role === 'user' ? 'user' : 'model'}\n${content}<end_of_turn>`,
    )
    .join('\n');

  const userBlock = `<start_of_turn>user\n${message}<end_of_turn>\n<start_of_turn>model\n`;

  return [systemBlock, historyBlock, userBlock].filter(Boolean).join('\n');
}

// ─── Provider ───────────────────────────────────────────────────────────────

export class LocalLiteRTProvider implements InferenceProvider {
  private currentRequestId: string | null = null;
  private subscriptions: ReturnType<NativeEventEmitter['addListener']>[] = [];

  async isAvailable(): Promise<boolean> {
    if (Platform.OS !== 'android' || !LiteRTLM) return false;
    try {
      return await LiteRTLM.isModelLoaded();
    } catch {
      return false;
    }
  }

  async generate(
    message: string,
    options: GenerateOptions,
  ): Promise<void> {
    if (!LiteRTLM) {
      options.onError('LiteRT-LM native module not available');
      return;
    }

    const isLoaded = await LiteRTLM.isModelLoaded();
    if (!isLoaded) {
      options.onError('模型未載入，請先在設定中選擇 Gemma 模型');
      return;
    }

    // 嘗試取得桌面 RAG context（有桌面連線時）
    let contextText: string | undefined;
    try {
      const { fetchRagContext } = await import('../desktop-api');
      const rag = await fetchRagContext(message);
      contextText = rag?.context_text;
    } catch {
      // 離線模式
    }

    const requestId = `req_${Date.now()}`;
    this.currentRequestId = requestId;
    this.cleanup();

    return new Promise<void>(resolve => {
      if (!emitter) {
        options.onError('EventEmitter not available');
        resolve();
        return;
      }

      this.subscriptions = [
        emitter.addListener('LiteRTLM_onToken', e => {
          if (e.requestId === requestId) options.onToken(e.token);
        }),
        emitter.addListener('LiteRTLM_onDone', e => {
          if (e.requestId === requestId) {
            options.onDone(e.fullText);
            this.cleanup();
            resolve();
          }
        }),
        emitter.addListener('LiteRTLM_onError', e => {
          if (e.requestId === requestId) {
            options.onError(e.error);
            this.cleanup();
            resolve();
          }
        }),
      ];

      const _settings = getLocalSettings();
      const prompt = buildPrompt(message, options.history, contextText);
      LiteRTLM.generateStream(prompt, requestId);
    });
  }

  cancel(): void {
    this.cleanup();
  }

  private cleanup() {
    this.subscriptions.forEach(s => s.remove());
    this.subscriptions = [];
    this.currentRequestId = null;
  }
}

// ─── Model Loader（由 SettingsScreen 呼叫）─────────────────────────────────

export async function loadGemmaModel(
  modelPath: string,
  maxTokens = 2048,
): Promise<void> {
  if (!LiteRTLM) throw new Error('LiteRT-LM not available on this platform');
  await LiteRTLM.loadModel(modelPath, maxTokens);
}

export async function unloadModel(): Promise<void> {
  if (LiteRTLM) await LiteRTLM.unloadModel();
}
