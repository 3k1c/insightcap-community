export type CaptureMethod = 'hotkey' | 'import' | 'mobile' | 'conversation';
export type ChunkType = 'text' | 'image' | 'document' | 'mixed' | 'conversation';
export type ChunkStatus = 'inbox' | 'processed' | 'archived' | 'auto_classified' | 'unclassified';
export type MessageRole = 'user' | 'assistant';
export type ModelProvider = 'ollama' | 'openai' | 'anthropic' | 'google' | 'xai' | 'openrouter';

export interface ProviderProfile {
    id: string;
    name: string;
    provider: ModelProvider;
    baseUrl?: string;
    apiKey?: string;
}

// ─── 知識庫 ─────────────────────────────────────────

export interface Space {
    id: string;
    name: string;
    description: string;
    color: string;
    rules: SpaceRule[];
    isSystem: boolean;
    parentId?: string;
    isPinned: boolean;
    isArchived: boolean;
    sortOrder: number;
    createdAt: string;
    updatedAt: string;
    chunkCount: number;
    status: string;
    projectSummary: string;
    createdBy: 'user' | 'ai';
}

export interface SpaceRule {
    type: 'exe_name' | 'url_domain' | 'app_name';
    value: string;
}

export interface Source {
    id: string;
    url?: string;
    filePath?: string;
    imagePath?: string;
    title: string;
    cleanContent: string;
    capturedAt: string;
    tags?: string | string[];
    spaceName?: string;
}

export interface Chunk {
    id: string;
    spaceId: string;
    sourceId?: string;
    conversationId?: string;
    type: ChunkType;
    rawContent: string;
    cleanContent: string;
    imagePath?: string;
    sourceApp: string;
    sourceExe: string;
    sourceUrl: string;
    sourceFilePath: string;
    captureMethod: CaptureMethod;
    sessionId: string;
    tags: string[];
    confidence: number;
    suggestedSpaceId?: string;
    status: ChunkStatus;
    chunkIndex: number;
    createdAt: string;
    updatedAt: string;
    knowledgeType: string;
    projectId?: string;
    placedBy: 'user' | 'ai';
}

export interface ConversationSummaryQueue {
    id: string;
    conversationId: string;
    triggeredBy: 'switch' | 'page_change' | 'minimize' | 'close';
    queuedAt: string;
    processed: boolean;
    processedAt?: string;
}

export interface TimelineItem {
    id: string;
    type: string;
    textPreview: string;
    imagePath?: string;
    filePath?: string;
    title: string;
    sourceApp: string;
    captureMethod: string;
    spaceId?: string;
    createdAt: string;
    sourceTable: string;
    fileSize?: number;
}

// ─── 設定 ─────────────────────────────────────────

export interface GeneralSettings {
    launchAtStartup: boolean;
    minimizeToTray: boolean;
    language: 'zh-TW' | 'zh-CN' | 'en';
}

export interface ModelSettings {
    provider: ModelProvider;
    model: string;
    apiKey?: string;
    baseUrl?: string;
}

export interface AIModelSettings {
    chatLlm: ModelSettings;
    contentProcessorLlm: ModelSettings;
    visionModel: ModelSettings;
    embeddingModel: ModelSettings;
    summaryModel?: 'follow_chat' | 'follow_content_processor';
    providerProfiles?: ProviderProfile[];
}

export interface KnowledgeSettings {
    kbPath: string;
    autoClassifyEnabled: boolean;
    notesFolder?: string; // 預設新增筆記的子資料夾（相對於 kbPath），預設為 "notes"
    autoSpaceMode: 'suggest' | 'auto'; // 'suggest' = toast 確認；'auto' = 直接建立
}

export interface HotkeySettings {
    captureClipboard: string;
    quickInput: string;
}

export interface AutoCleanupSettings {
    enabled: boolean;
    retentionDays: number;
}

export interface WebSearchSettings {
    enabled: boolean;
    provider: string;
    apiKey: string;
}

export interface SpaceSuggestion {
    chunkIds: string[];
    suggestedName: string;
    sampleContent: string[];
}

export interface EditorSettings {
    /** 預設字型，空字串代表使用系統預設 */
    defaultFont: string;
    /** 預設字號，如 '12' */
    defaultFontSize: string;
    /** 預設行距，如 '1.5' */
    defaultLineSpacing: string;
    /** Ctrl+S 及匯出選單的預設格式 */
    defaultExportFormat: 'docx' | 'md' | 'txt';
    /** 匯出檔案的子目錄（相對於文件所在資料夾），如 'exports' */
    exportSubdir: string;
}

export interface AllSettings {
    general: GeneralSettings;
    aiModels: AIModelSettings;
    knowledge: KnowledgeSettings;
    hotkeys: HotkeySettings;
    autoCleanup: AutoCleanupSettings;
    webSearch: WebSearchSettings;
    editor: EditorSettings;
    bilibiliSessdata?: string | null;
    lastOpenFile?: string | null;
    lastOpenConv?: string | null;
}

// ─── 編輯器模組 ─────────────────────────────────────────

export interface DocumentInfo {
    path: string;
    isUnsaved: boolean;
}

// ─── Context Scan ─────────────────────────────────────────

export interface ContextGroup {
    knowledgeType: 'data' | 'pattern' | 'log';
    count: number;
    topTags: string[];
    latestAt: string;
}

export interface RelatedContextSummary {
    hasContext: boolean;
    groups: ContextGroup[];
    contextHint: string;
}

// ─── 外部知識庫 ─────────────────────────────────────────
export type KbType = 'general' | 'legal' | 'finance' | 'training' | 'sales';

export interface KbMetadata {
    kbVersion: string;
    kbType: KbType;
    embeddingModel: string;
    embeddingDimension: number;
    createdBy: string;
    description: string;
    isReadonly: boolean;
}

export interface ExternalKnowledgeBase {
    id: string;
    name: string;
    dbPath: string;
    kbType: string;
    embeddingModel: string;
    embeddingDimension: number;
    createdBy: string;
    description: string;
    status: 'connected' | 'disconnected' | 'incompatible';
    lastChecked: string | null;
    loadedAt: string;
    updatedAt: string;
}

export interface ExternalKbLoadResult {
    success: boolean;
    reason?: string;
    metadata?: KbMetadata;
}
