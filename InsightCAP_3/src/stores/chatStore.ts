import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface Conversation {
    id: string;
    title: string;
    summary: string;
    projectId?: string | null;
    isPinned: boolean;
    isLocked: boolean;
    createdAt: string;
    updatedAt: string;
}

export interface Project {
    id: string;
    name: string;
    defaultTags: string[];
    color?: string | null;
    isPinned: boolean;
    isArchived: boolean;
    sortOrder: number;
    createdAt: string;
    updatedAt: string;
}

export interface Message {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    created_at: string;
    attachedFiles?: { name: string; filePath: string; fileType: string; previewUrl?: string }[];
    mentionedSources?: { id: string; title: string }[];
    citationSources?: string[];
}

interface ContextStats {
    dataCount: number;
    patternCount: number;
    logCount: number;
}

interface ChatState {
    conversations: Conversation[];
    projects: Project[];
    activeConversationId: string | null;
    activeProjectId: string | null;
    messages: Message[];
    isGenerating: boolean;
    streamingContent: string;         // 當前 streaming 中的 assistant 回覆
    contextStats: ContextStats;
    expandedProjectIds: Set<string>;
    // 對話級附件 ID：附加文件在整個對話中持續有效
    conversationTempChunkIds: string[];

    loadConversations: () => Promise<void>;
    loadProjects: () => Promise<void>;
    createNewConversation: (projectId?: string) => Promise<string | null>;
    renameConversation: (conversationId: string, title: string) => Promise<void>;
    deleteConversation: (conversationId: string) => Promise<void>;
    updateConversation: (conversationId: string, isPinned?: boolean, isLocked?: boolean) => Promise<void>;
    createProject: (name: string, color?: string) => Promise<void>;
    updateProject: (projectId: string, name?: string, color?: string, isPinned?: boolean) => Promise<void>;
    deleteProject: (projectId: string) => Promise<void>;
    moveConversationToProject: (conversationId: string, projectId?: string) => Promise<void>;
    reorderProjects: (orderedIds: string[]) => Promise<void>;
    toggleProjectExpanded: (projectId: string) => void;
    loadMessages: (conversationId: string) => Promise<void>;
    sendMessage: (content: string, opts?: {
        ragEnabled?: boolean;
        webEnabled?: boolean;
        mentionedSourceIds?: string[];
        mentionedTagNames?: string[];
        attachedFiles?: { name: string; filePath: string; fileType: string; previewUrl?: string; tempChunkIds?: string[] }[];
        tempChunkIds?: string[];
        thinkingMode?: 'normal' | 'think';
    }) => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
    conversations: [],
    projects: [],
    activeConversationId: null,
    activeProjectId: null,
    messages: [],
    isGenerating: false,
    streamingContent: '',
    expandedProjectIds: new Set(),
    contextStats: { dataCount: 0, patternCount: 0, logCount: 0 },
    conversationTempChunkIds: [],

    loadProjects: async () => {
        try {
            console.log('[chatStore] Fetching projects...');
            const projects = await invoke<Project[]>('get_projects');
            console.log('[chatStore] Received projects:', projects);
            set({ projects });
            console.log('[chatStore] Projects state updated');
        } catch (error) {
            console.error('[chatStore] Failed to load projects:', error);
        }
    },

    loadConversations: async () => {
        try {
            const conversations = await invoke<any[]>('get_conversations');
            // Map camelCase from backend to our interface
            const mapped = conversations.map(c => ({
                id: c.id,
                title: c.title,
                summary: c.summary,
                projectId: c.projectId || c.project_id,
                isPinned: c.isPinned ?? c.is_pinned ?? false,
                isLocked: c.isLocked ?? c.is_locked ?? false,
                createdAt: c.createdAt || c.created_at,
                updatedAt: c.updatedAt || c.updated_at,
            }));
            set({ conversations: mapped });

            const { activeConversationId } = get();
            if (!activeConversationId && mapped.length > 0) {
                set({ activeConversationId: mapped[0].id });
                get().loadMessages(mapped[0].id);
            }
        } catch (error) {
            console.error('Failed to load conversations:', error);
        }
    },

    createNewConversation: async (projectId?: string) => {
        try {
            const newId = await invoke<string>('create_conversation', {
                projectId: projectId ?? null,
            });
            await get().loadConversations();
            set({ activeConversationId: newId, activeProjectId: projectId ?? null, messages: [] });
            return newId;
        } catch (error) {
            console.error('Failed to create conversation:', error);
            return null;
        }
    },

    renameConversation: async (conversationId: string, title: string) => {
        await invoke('rename_conversation', { conversationId, title });
        set(state => ({
            conversations: state.conversations.map(c =>
                c.id === conversationId ? { ...c, title } : c
            ),
        }));
    },

    deleteConversation: async (conversationId: string) => {
        await invoke('delete_conversation', { conversationId });
        set(state => {
            const conversations = state.conversations.filter(c => c.id !== conversationId);
            const activeConversationId = state.activeConversationId === conversationId
                ? (conversations[0]?.id ?? null)
                : state.activeConversationId;
            return { conversations, activeConversationId, messages: activeConversationId === state.activeConversationId ? state.messages : [] };
        });
    },

    updateConversation: async (conversationId: string, isPinned?: boolean, isLocked?: boolean) => {
        await invoke('update_conversation', { conversationId, isPinned, isLocked });
        set(state => ({
            conversations: state.conversations.map(c =>
                c.id === conversationId
                    ? { ...c, ...(isPinned !== undefined && { isPinned }), ...(isLocked !== undefined && { isLocked }) }
                    : c
            ),
        }));
    },

    createProject: async (name: string, color?: string) => {
        try {
            console.log('[chatStore] Invoking create_project with:', { name, color });
            const result = await invoke('create_project', { 
                name,
                color 
            });
            console.log('[chatStore] create_project result:', result);
            
            console.log('[chatStore] Reloading projects after creation');
            await get().loadProjects();
            console.log('[chatStore] Projects reloaded, current projects:', get().projects);
        } catch (error) {
            console.error('[chatStore] Failed to create project:', error);
            throw error;
        }
    },

    updateProject: async (projectId: string, name?: string, color?: string, isPinned?: boolean) => {
        try {
            await invoke('update_project', { projectId, name, color, isPinned });
            await get().loadProjects();
        } catch (error) {
            console.error('Failed to update project:', error);
        }
    },

    deleteProject: async (projectId: string) => {
        try {
            await invoke('delete_project', { projectId });
            await get().loadProjects();
            await get().loadConversations();
        } catch (error) {
            console.error('Failed to delete project:', error);
        }
    },

    moveConversationToProject: async (conversationId: string, projectId?: string) => {
        try {
            await invoke('move_conversation_to_project', { conversationId, projectId });
            await get().loadConversations();
        } catch (error) {
            console.error('Failed to move conversation:', error);
        }
    },

    reorderProjects: async (orderedIds: string[]) => {
        // 樂觀更新：先在前端重排
        set(state => {
            const map = new Map(state.projects.map(p => [p.id, p]));
            const reordered = orderedIds
                .map((id, i) => map.get(id) ? { ...map.get(id)!, sortOrder: i * 1000 } : null)
                .filter(Boolean) as typeof state.projects;
            return { projects: reordered };
        });
        // 逐一更新後端
        try {
            await Promise.all(
                orderedIds.map((id, i) =>
                    invoke('update_project_sort_order', { projectId: id, sortOrder: i * 1000 })
                )
            );
        } catch (error) {
            console.error('Failed to reorder projects:', error);
            await get().loadProjects();
        }
    },

    toggleProjectExpanded: (projectId: string) => {
        set(state => {
            const newSet = new Set(state.expandedProjectIds);
            if (newSet.has(projectId)) {
                newSet.delete(projectId);
            } else {
                newSet.add(projectId);
            }
            return { expandedProjectIds: newSet };
        });
    },

    loadMessages: async (conversationId: string) => {
        try {
            const currentId = get().activeConversationId;
            if (currentId && currentId !== conversationId) {
                invoke('enqueue_summary', { conversationId: currentId, triggerType: 'switch' }).catch(e => console.warn('Summary enqueue failed:', e));
            }
            const conv = get().conversations.find(c => c.id === conversationId);
            // 切換對話時先清空，稍後從 metadata 恢復
            set({ activeConversationId: conversationId, activeProjectId: conv?.projectId ?? null, conversationTempChunkIds: [], streamingContent: '' });
            const raw = await invoke<(Message & { metadata?: string })[]>('get_messages', { conversationId });
            let restoredTempChunkIds: string[] = [];
            const messages: Message[] = raw.map(m => {
                let attachedFiles: Message['attachedFiles'];
                let citationSources: Message['citationSources'];
                let mentionedSources: Message['mentionedSources'];
                try {
                    const meta = JSON.parse(m.metadata || '{}');
                    if (Array.isArray(meta.attachedFiles)) attachedFiles = meta.attachedFiles;
                    if (Array.isArray(meta.citationSources)) citationSources = meta.citationSources;
                    if (Array.isArray(meta.mentionedSources)) mentionedSources = meta.mentionedSources;
                    // 從最後一則含 tempChunkIds 的 user 訊息恢復（累積式，取最新即可）
                    if (m.role === 'user' && Array.isArray(meta.tempChunkIds)) {
                        restoredTempChunkIds = meta.tempChunkIds;
                    }
                } catch { /* ignore */ }
                return { ...m, attachedFiles, mentionedSources, citationSources };
            });
            set({ messages, conversationTempChunkIds: restoredTempChunkIds });
        } catch (error) {
            console.error('Failed to load messages:', error);
        }
    },

    sendMessage: async (content: string, opts?: {
        ragEnabled?: boolean;
        webEnabled?: boolean;
        mentionedSourceIds?: string[];
        mentionedSources?: { id: string; title: string }[];
        mentionedTagNames?: string[];
        attachedFiles?: { name: string; filePath: string; fileType: string; previewUrl?: string; tempChunkIds?: string[] }[];
        tempChunkIds?: string[];
        thinkingMode?: 'normal' | 'think';
    }) => {
        const { activeConversationId, activeProjectId, messages } = get();
        if (!activeConversationId) return;

        set({ isGenerating: true, streamingContent: '' });
        try {
            // 1. 累積對話級 tempChunkIds（本輪新增的附件合併到對話級）
            const newChunkIds = opts?.tempChunkIds ?? [];
            const allTempChunkIds = [...get().conversationTempChunkIds, ...newChunkIds];
            if (newChunkIds.length > 0) {
                set({ conversationTempChunkIds: allTempChunkIds });
            }

            // 2. Optimistically append user message
            const userMsg: Message = {
                id: 'optimistic-user-' + Date.now(),
                role: 'user',
                content,
                created_at: new Date().toISOString(),
                attachedFiles: opts?.attachedFiles,
                mentionedSources: opts?.mentionedSources?.length ? opts.mentionedSources : undefined,
            };
            set(state => ({ messages: [...state.messages, userMsg] }));

            // 3. Save user message to DB（含 tempChunkIds 以供重整後恢復）
            const metaObj: Record<string, unknown> = {};
            if (opts?.attachedFiles?.length) {
                metaObj.attachedFiles = opts.attachedFiles.map(f => ({ name: f.name, filePath: f.filePath, fileType: f.fileType, previewUrl: f.previewUrl }));
            }
            if (opts?.mentionedSources?.length) {
                metaObj.mentionedSources = opts.mentionedSources;
            }
            if (allTempChunkIds.length > 0) {
                metaObj.tempChunkIds = allTempChunkIds;
            }
            const metadata = Object.keys(metaObj).length > 0 ? JSON.stringify(metaObj) : undefined;
            await invoke('add_message', {
                conversationId: activeConversationId,
                role: 'user',
                content,
                metadata,
            });

            // 4. 組裝歷史：只傳最近 6 輪（12 條）原文
            //    超出部分已由 conversation.summary 代表，注入為 system context
            const history: [string, string][] = messages
                .filter(m => m.role === 'user' || m.role === 'assistant')
                .slice(-12)
                .map(m => [m.role, m.content]);

            // 取得當前對話的 summary（代表 12 條之前的所有歷史）
            const activeConv = get().conversations.find(c => c.id === activeConversationId);
            const conversationSummary = activeConv?.summary?.trim() || null;

            const ragEnabled = opts?.ragEnabled ?? true;
            const placeholderMsgId = 'streaming-assistant-' + Date.now();

            // 5. 加入空白 assistant 佔位，準備 streaming 填入
            set(state => ({
                messages: [...state.messages, {
                    id: placeholderMsgId,
                    role: 'assistant' as const,
                    content: '',
                    created_at: new Date().toISOString(),
                }]
            }));

            // 6. 訂閱 streaming events
            let accumulated = '';
            const unlistenToken = await listen<{ conversationId: string; token: string }>(
                'rag-stream-token',
                (event) => {
                    if (event.payload.conversationId !== activeConversationId) return;
                    accumulated += event.payload.token;
                    set(state => ({
                        streamingContent: accumulated,
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, content: accumulated } : m
                        ),
                    }));
                }
            );

            const unlistenDone = await listen<{ conversationId: string; fullAnswer: string; citationSources?: string[] }>(
                'rag-stream-done',
                (event) => {
                    if (event.payload.conversationId !== activeConversationId) return;
                    const finalAnswer = event.payload.fullAnswer || accumulated;
                    const citationSources = Array.isArray(event.payload.citationSources) ? event.payload.citationSources : [];

                    // 7. 先更新 UI，再非同步存 DB
                    set(state => ({
                        isGenerating: false,
                        streamingContent: '',
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, content: finalAnswer, citationSources } : m
                        ),
                    }));

                    invoke('add_message', {
                        conversationId: activeConversationId,
                        role: 'assistant',
                        content: finalAnswer,
                        metadata: JSON.stringify({ citationSources }),
                    }).catch(e => console.error('Failed to save assistant message:', e));

                    unlistenToken();
                    unlistenDone();
                }
            );

            // 8. 呼叫 streaming command，加 120s timeout 保護
            const timeoutPromise = new Promise<never>((_, reject) =>
                setTimeout(() => reject(new Error('LLM response timeout')), 120_000)
            );

            const streamPromise = invoke('rag_query_stream', {
                query: content,
                conversationId: activeConversationId,
                history,
                conversationSummary,
                projectId: activeProjectId ?? null,
                sourceIds: opts?.mentionedSourceIds?.length ? opts.mentionedSourceIds : null,
                tagFilter: opts?.mentionedTagNames?.length ? opts.mentionedTagNames : null,
                ragEnabled,
                webEnabled: opts?.webEnabled ?? false,
                tempChunkIds: allTempChunkIds.length > 0 ? allTempChunkIds : null,
                thinkingMode: opts?.thinkingMode ?? 'normal',
            });

            Promise.race([streamPromise, timeoutPromise]).catch(async (err) => {
                console.error('rag_query_stream failed:', err);
                unlistenToken();
                unlistenDone();

                // fallback：用 non-streaming
                try {
                    const ragResponse = await invoke<any>('rag_query', {
                        query: content,
                        history,
                        conversationSummary,
                        projectId: activeProjectId ?? null,
                        sourceIds: opts?.mentionedSourceIds?.length ? opts.mentionedSourceIds : null,
                        tagFilter: opts?.mentionedTagNames?.length ? opts.mentionedTagNames : null,
                        ragEnabled,
                        webEnabled: opts?.webEnabled ?? false,
                        tempChunkIds: allTempChunkIds.length > 0 ? allTempChunkIds : null,
                        thinkingMode: opts?.thinkingMode ?? 'normal',
                    });
                    const fallbackAnswer = ragResponse.answer || '（無回應）';
                    const fallbackCitations = Array.isArray(ragResponse.citationSources) ? ragResponse.citationSources : [];
                    await invoke('add_message', {
                        conversationId: activeConversationId,
                        role: 'assistant',
                        content: fallbackAnswer,
                        metadata: JSON.stringify({ citationSources: fallbackCitations }),
                    });
                    set(state => ({
                        isGenerating: false,
                        streamingContent: '',
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, content: fallbackAnswer, citationSources: fallbackCitations } : m
                        ),
                    }));
                } catch (fallbackErr) {
                    const errMsg = `（回應失敗：${fallbackErr}）`;
                    set(state => ({
                        isGenerating: false,
                        streamingContent: '',
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, content: errMsg } : m
                        ),
                    }));
                }
            });

        } catch (error) {
            console.error('Failed to send message:', error);
            set({ isGenerating: false, streamingContent: '' });
        }
    },
}));
