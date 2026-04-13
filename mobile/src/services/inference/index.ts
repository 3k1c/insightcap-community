/**
 * InsightCAP Mobile — 推理 Provider 工廠
 *
 * 三種模式：
 *   'local'   — LiteRT-LM on-device (Gemma 4)
 *   'cloud'   — 雲端 API 直連（Claude / OpenAI / Gemini）
 *   'desktop' — 桌面代理 SSE（複用桌面 LLM + 知識庫）
 */

export type InferenceMode = 'local' | 'cloud' | 'desktop';

export interface GenerateOptions {
  history?: Array<{ role: 'user' | 'assistant'; content: string }>;
  onToken: (token: string) => void;
  onDone: (fullText: string) => void;
  onError: (error: string) => void;
}

export interface InferenceProvider {
  isAvailable(): Promise<boolean>;
  generate(message: string, options: GenerateOptions): Promise<void>;
  cancel(): void;
}

export async function createProvider(
  mode: InferenceMode,
): Promise<InferenceProvider> {
  switch (mode) {
    case 'local': {
      const { LocalLiteRTProvider } = await import('./local-litert-lm');
      return new LocalLiteRTProvider();
    }
    case 'cloud': {
      const { CloudApiProvider } = await import('./cloud-api');
      return new CloudApiProvider();
    }
    case 'desktop': {
      const { DesktopRelayProvider } = await import('./desktop-relay');
      return new DesktopRelayProvider();
    }
  }
}
