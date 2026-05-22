import { invoke } from '@tauri-apps/api/core';
import type { PdfExportBlock } from './editor-export';
import type { AllSettings, Space, Chunk, ChunkStatus, Source, TimelineItem, SpaceSuggestion, RelatedContextSummary, ExternalKnowledgeBase, ExternalKbLoadResult } from './types';

export interface Folder { id: string; name: string; }
export interface Conversation { id: string; title: string; }

export const tauriCmd = {
    getSettings: async (): Promise<AllSettings> => {
        return invoke('get_settings');
    },

    saveSettings: async (settings: AllSettings): Promise<void> => {
        return invoke('save_settings', { settings });
    },

    getCaptureTimeline: async (params: { sourceFilter?: string; typeFilter?: string; spaceFilter?: string; limit?: number; offset?: number }): Promise<TimelineItem[]> => {
        return invoke('get_capture_timeline', {
            sourceFilter: params.sourceFilter ?? null,
            typeFilter: params.typeFilter ?? null,
            spaceFilter: params.spaceFilter ?? null,
            limit: params.limit ?? null,
            offset: params.offset ?? null
        });
    },

    getSpaces: async (): Promise<Space[]> => {
        return invoke('get_spaces');
    },

    getArchivedSpaces: async (): Promise<Space[]> => {
        return invoke('get_archived_spaces');
    },

    createSpace: async (name: string, description: string, color: string): Promise<Space> => {
        return invoke('create_space', { name, description, color });
    },

    updateSpace: async (id: string, name?: string, description?: string, color?: string, isPinned?: boolean, isArchived?: boolean, sortOrder?: number): Promise<void> => {
        return invoke('update_space', {
            id,
            name: name ?? null,
            description: description ?? null,
            color: color ?? null,
            isPinned: isPinned ?? null,
            isArchived: isArchived ?? null,
            sortOrder: sortOrder ?? null
        });
    },

    deleteSpace: async (id: string): Promise<void> => {
        return invoke('delete_space', { id });
    },

    mergeSpaces: async (sourceId: string, targetId: string): Promise<void> => {
        return invoke('merge_spaces', { sourceId, targetId });
    },

    getChunks: async (params: { spaceId?: string; status?: ChunkStatus; limit?: number }): Promise<Chunk[]> => {
        return invoke('get_chunks', params);
    },

    getSource: async (sourceId: string): Promise<Source> => {
        return invoke('get_source', { sourceId });
    },

    getInboxCount: async (): Promise<number> => {
        return invoke('get_inbox_count');
    },

    updateChunk: async (chunkId: string, cleanContent: string, tags: string[], knowledgeType: string): Promise<void> => {
        return invoke('update_chunk', { chunkId, cleanContent, tags, knowledgeType });
    },

    deleteChunk: async (chunkId: string): Promise<void> => {
        return invoke('delete_chunk', { chunkId });
    },

    summarizeSearchResults: async (snippets: string[], query: string): Promise<string> => {
        return invoke('summarize_search_results', { snippets, query });
    },

    captureUrl: async (url: string, conversationId?: string): Promise<void> => {
        return invoke('capture_url', { url, conversationId: conversationId ?? null });
    },

    ingestFile: async (filePath: string, conversationId?: string, importTaskId?: string): Promise<void> => {
        return invoke('ingest_file', { filePath, conversationId: conversationId ?? null, importTaskId: importTaskId ?? null });
    },

    cancelProcessingTask: async (taskId: string): Promise<void> => {
        return invoke('cancel_processing_task', { taskId });
    },

    ingestScreenshot: async (conversationId?: string): Promise<void> => {
        return invoke('ingest_screenshot', { conversationId: conversationId ?? null });
    },

    attachChunkToConversation: async (chunkId: string, conversationId: string): Promise<void> => {
        return invoke('attach_chunk_to_conversation', { chunkId, conversationId });
    },

    acceptChunkSuggestion: async (chunkId: string): Promise<void> => {
        return invoke('accept_chunk_suggestion', { chunkId });
    },

    rejectChunkSuggestion: async (chunkId: string): Promise<void> => {
        return invoke('reject_chunk_suggestion', { chunkId });
    },

    moveChunkToSpace: async (chunkId: string, spaceId: string): Promise<void> => {
        return invoke('move_chunk_to_space', { chunkId, spaceId });
    },

    dismissChunkSuggestion: async (chunkId: string): Promise<void> => {
        return invoke('dismiss_chunk_suggestion', { chunkId });
    },

    searchSources: async (query: string, limit?: number): Promise<Array<{
        id: string;
        title: string;
        preview: string;
        url?: string;
        filePath?: string;
        capturedAt: string;
        sourceType: 'url' | 'file' | 'text';
    }>> => {
        return invoke('search_sources', { query, limit: limit ?? null });
    },


    updateFolder: async (id: string, name?: string, isPinned?: number, sortOrder?: number, isArchived?: number): Promise<void> => {
        return invoke('update_folder', {
            id,
            name: name ?? null,
            isPinned: isPinned ?? null,
            sortOrder: sortOrder ?? null,
            isArchived: isArchived ?? null
        });
    },

    deleteFolder: async (id: string): Promise<void> => {
        return invoke('delete_folder', { id });
    },

    updateProjectQueryScope: async (folderId: string, queryScope: string | null): Promise<void> => {
        return invoke('update_project_query_scope', { folderId, queryScope });
    },

    checkPatternRecall: async (conversationId: string): Promise<Array<{
        id: string;
        knowledge_type: string;
        content: string;
        space_name?: string;
    }>> => {
        return invoke('check_pattern_recall', { conversationId });
    },

    getArchivedFolders: async (): Promise<Folder[]> => {
        return invoke('get_archived_folders');
    },

    getArchivedConversations: async (): Promise<Conversation[]> => {
        return invoke('get_archived_conversations');
    },

    updateConversation: async (id: string, title?: string, folderId?: string, isLocked?: number, isPinned?: number, isArchived?: number): Promise<void> => {
        return invoke('update_conversation', {
            id,
            title: title ?? null,
            folderId: folderId ?? null,
            isLocked: isLocked ?? null,
            isPinned: isPinned ?? null,
            isArchived: isArchived ?? null
        });
    },

    deleteConversation: async (id: string): Promise<void> => {
        return invoke('delete_conversation', { id });
    },

    reorderConversations: async (ids: string[], folderId: string | null): Promise<void> => {
        return invoke('reorder_conversations', { ids, folderId });
    },
    reorderFolders: async (ids: string[]): Promise<void> => {
        return invoke('reorder_folders', { ids });
    },
    reorderSpaces: async (ids: string[]): Promise<void> => {
        return invoke('reorder_spaces', { ids });
    },

    pinConversation: async (id: string): Promise<void> => {
        return invoke('update_conversation', { id, isPinned: 1, isLocked: null });
    },

    unpinConversation: async (id: string): Promise<void> => {
        return invoke('update_conversation', { id, isPinned: 0, isLocked: null });
    },

    lockConversation: async (id: string): Promise<void> => {
        return invoke('update_conversation', { id, isPinned: null, isLocked: 1 });
    },

    unlockConversation: async (id: string): Promise<void> => {
        return invoke('update_conversation', { id, isPinned: null, isLocked: 0 });
    },

    exportKnowledgeBase: async (destDir: String): Promise<void> => {
        return invoke('export_knowledge_base', { destDir });
    },

    clearKnowledgeBase: async (): Promise<void> => {
        return invoke('clear_knowledge_base');
    },

    rebuildIndex: async (): Promise<void> => {
        return invoke('rebuild_index');
    },

    initializeWorkspace: async (path: string): Promise<void> => {
        return invoke('initialize_workspace', { path });
    },

    switchKbPath: async (newPath: string): Promise<void> => {
        return invoke('switch_kb_path', { newPath });
    },

    acknowledgeKbConflict: async (): Promise<void> => {
        return invoke('acknowledge_kb_conflict');
    },

    enqueueConversationSummary: async (params: {
        conversationId: string;
        triggeredBy: 'switch' | 'page_change' | 'minimize' | 'close' | 'minimize_or_close';
        force?: boolean; // For testing
    }): Promise<string> => {
        return invoke<string>('enqueue_conversation_summary', {
            conversationId: params.conversationId,
            triggeredBy: params.triggeredBy,
            force: params.force ?? null,
        });
    },

    processConversationSummaryQueue: async (): Promise<number> => {
        return invoke<number>('process_conversation_summary_queue');
    },

    setCurrentConversation: async (params: {
        id: string | null;
    }): Promise<void> => {
        return invoke('set_current_conversation', { id: params.id });
    },

    saveTextToKnowledge: async (params: {
        content: string;
        spaceId: string;
        captureMethod: string;
        sourceConversationId?: string;
        knowledgeType?: string;
        projectId?: string;
    }): Promise<string> => {
        return invoke<string>('save_text_to_knowledge', {
            content: params.content,
            spaceId: params.spaceId,
            captureMethod: params.captureMethod,
            sourceConversationId: params.sourceConversationId ?? null,
            knowledgeType: params.knowledgeType ?? null,
            projectId: params.projectId ?? null,
        });
    },
    openBilibiliLogin: async (): Promise<string> => {
        return invoke('open_bilibili_login');
    },

    getAttachmentStats: async (): Promise<{
        totalSizeBytes: number;
        fileCount: number;
        typeCounts: Record<string, number>;
        orphanedCount: number;
    }> => {
        return invoke('get_attachment_stats');
    },

    cleanupOrphanedAttachments: async (): Promise<number> => {
        return invoke('cleanup_orphaned_attachments');
    },

    openFileInSystem: async (path: string): Promise<void> => {
        return invoke('open_file_in_system', { path });
    },

    testOllama: async (baseUrl: string): Promise<boolean> => {
        return invoke('test_ollama', { baseUrl });
    },

    pullOllamaModel: async (model: string): Promise<void> => {
        return invoke('pull_ollama_model', { model });
    },

    getOllamaPullStatus: async (): Promise<Record<string, {
        status: 'pulling' | 'completed' | 'error';
        progress?: number;
        error?: string;
    }>> => {
        return invoke('get_ollama_pull_status');
    },

    testTavily: async (apiKey: string): Promise<boolean> => {
        return invoke('test_tavily', { apiKey });
    },

    testProviderConnection: async (provider: string, baseUrl: string, apiKey: string): Promise<boolean> => {
        return invoke('test_provider_connection', { provider, baseUrl, apiKey });
    },

    exportWorkspace: async (destPath: string): Promise<void> => {
        return invoke('export_workspace', { destPath });
    },

    importWorkspace: async (zipPath: string, destDir: string): Promise<void> => {
        return invoke('import_workspace', { zipPath, destDir });
    },

    updateGlobalHotkey: async (hotkeyStr: string): Promise<void> => {
        return invoke('update_global_hotkey', { hotkeyStr });
    },
    updateQuickInputHotkey: async (hotkeyStr: string): Promise<void> => {
        return invoke('update_quick_input_hotkey', { hotkeyStr });
    },

    analyzeSpaceSuggestions: async (): Promise<SpaceSuggestion | null> => {
        return invoke('analyze_space_suggestions');
    },

    applySpaceSuggestion: async (name: string, color: string, chunkIds: string[]): Promise<void> => {
        return invoke('apply_space_suggestion', { name, color, chunkIds });
    },

    exportDocument: async (absolutePath: string, content: string): Promise<void> => {
        return invoke('export_document', { absolutePath, content });
    },

    exportPdfText: async (absolutePath: string, content: string): Promise<void> => {
        return invoke('export_pdf_text', { absolutePath, content });
    },

    exportPdfDocument: async (absolutePath: string, blocks: PdfExportBlock[]): Promise<void> => {
        return invoke('export_pdf_document', { absolutePath, blocks });
    },

    exportDocx: async (absolutePath: string, markdown: string): Promise<void> => {
        return invoke('export_docx', { absolutePath, markdown });
    },

    writeBinaryFile: async (absolutePath: string, base64Content: string): Promise<void> => {
        return invoke('write_binary_file', { absolutePath, base64Content });
    },

    exportXlsx: async (absolutePath: string, markdown: string): Promise<void> => {
        return invoke('export_xlsx', { absolutePath, markdown });
    },

    createDocument: async (path: string, format: string): Promise<void> => {
        return invoke('create_document', { path, format });
    },

    openDocument: async (path: string): Promise<string> => {
        return invoke('open_document', { path });
    },

    saveDocument: async (path: string, content: string): Promise<void> => {
        return invoke('save_document', { path, content });
    },

    listDocuments: async (subDir?: string): Promise<Array<{ path: string; name: string; modifiedAt: string }>> => {
        return invoke('list_documents', { subDir: subDir ?? null });
    },

    renameDocument: async (oldPath: string, newPath: string): Promise<void> => {
        return invoke('rename_document', { oldPath, newPath });
    },

    getLastOpenFile: async (): Promise<string | null> => {
        return invoke('get_last_open_file');
    },

    setLastOpenFile: async (path: string | null): Promise<void> => {
        return invoke('set_last_open_file', { path });
    },

    readImageBase64: async (absolutePath: string): Promise<string> => {
        return invoke('read_image_base64', { absolutePath });
    },

    copyImageToAssets: async (docPath: string, imageAbsPath: string): Promise<string> => {
        return invoke('copy_image_to_assets', { docPath, imageAbsPath });
    },

    copyEditorImageToAssets: async (imageAbsPath: string): Promise<string> => {
        return invoke('copy_editor_image_to_assets', { imageAbsPath });
    },

    saveEditorToKnowledge: async (title: string, content: string): Promise<void> => {
        return invoke('save_editor_to_knowledge', { title, content });
    },

    loadExternalKb: async (dbPath: string): Promise<ExternalKbLoadResult> => {
        return invoke('load_external_kb', { dbPath });
    },

    removeExternalKb: async (ekbId: string): Promise<void> => {
        return invoke('remove_external_kb', { ekbId });
    },

    getExternalKbs: async (): Promise<ExternalKnowledgeBase[]> => {
        return invoke('get_external_kbs');
    },

    recheckExternalKbs: async (): Promise<void> => {
        return invoke('recheck_external_kbs');
    },

    scanConversationContext: async (
        conversationId: string,
        userMessage: string
    ): Promise<RelatedContextSummary> => {
        return invoke('scan_conversation_context', {
            conversationId,
            userMessage,
        });
    },

    getAuthStatus: async (kbPath: string): Promise<{ isSetup: boolean; autoLogin: boolean }> => {
        return invoke('get_auth_status', { kbPath });
    },

    setupAuth: async (payload: {
        displayName: string;
        password: string;
        autoLogin: boolean;
        kbPath: string;
    }): Promise<string> => {
        return invoke('setup_auth', { payload });
    },

    tryAutoLogin: async (kbPath: string): Promise<boolean> => {
        return invoke('try_auto_login', { kbPath });
    },

    login: async (payload: { password: string; kbPath: string }): Promise<void> => {
        return invoke('login', { payload });
    },

    getLockStatus: async (kbPath: string): Promise<{
        isLocked: boolean;
        isPermanentlyLocked: boolean;
        remainingSecs: number | null;
        failCount: number;
    }> => {
        return invoke('get_lock_status', { kbPath });
    },

    changePassword: async (payload: {
        oldPassword: string;
        newPassword: string;
        kbPath: string;
    }): Promise<string> => {
        return invoke('change_password', { payload });
    },

    confirmNewRecovery: async (kbPath: string): Promise<void> => {
        return invoke('confirm_new_recovery', { kbPath });
    },

    recoverWithMnemonic: async (payload: {
        mnemonic: string;
        newPassword: string;
        kbPath: string;
    }): Promise<string> => {
        return invoke('recover_with_mnemonic', { payload });
    },

    getPendingRecovery: async (): Promise<string | null> => {
        return invoke('get_pending_recovery');
    },

    restartApp: async (): Promise<void> => {
        return invoke('restart_app');
    },
};
