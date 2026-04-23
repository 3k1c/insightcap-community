import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import i18n from '../i18n';

function recentAssistantContext(messages: Message[]): string {
    return messages
        .filter(m => m.role === 'assistant')
        .slice(-3)
        .map(m => m.content)
        .join('\n');
}

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
    reasoningContent?: string;
}

export interface ContextStats {
    dataCount: number;
    patternCount: number;
    logCount: number;
    patternHints: string[];
    logHints: string[];
}

interface ChatState {
    conversations: Conversation[];
    projects: Project[];
    activeConversationId: string | null;
    activeProjectId: string | null;
    messages: Message[];
    isGenerating: boolean;
    streamingContent: string; // Current assistant response in streaming mode
    contextStats: ContextStats;
    expandedProjectIds: Set<string>;
    conversationTempChunkIds: string[];
    pendingReminderAckByConversation: Record<string, boolean>;

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
    triggerUrgentReminderCheck: (conversationId: string, userMsg: string, aiMsg: string) => Promise<void>;
    sendMessage: (content: string, opts?: {
        ragEnabled?: boolean;
        webEnabled?: boolean;
        mentionedSourceIds?: string[];
        mentionedTagNames?: string[];
        attachedFiles?: { name: string; filePath: string; fileType: string; previewUrl?: string; tempChunkIds?: string[] }[];
        tempChunkIds?: string[];
        thinkingMode?: 'normal' | 'think';
    }) => Promise<void>;
    autoTitleConversation: (conversationId: string) => Promise<void>;
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
    contextStats: { dataCount: 0, patternCount: 0, logCount: 0, patternHints: [], logHints: [] },
    conversationTempChunkIds: [],
    pendingReminderAckByConversation: {},

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

    autoTitleConversation: async (conversationId: string) => {
        try {
            const conv = get().conversations.find(c => c.id === conversationId);
            if (!conv || conv.title !== '\u65b0\u5c0d\u8a71') return; // Already renamed; skip auto-title.
            const title = await invoke<string>('auto_title_conversation', { conversationId });
            if (title && title !== '\u65b0\u5c0d\u8a71') {
                set(state => ({
                    conversations: state.conversations.map(c =>
                        c.id === conversationId ? { ...c, title } : c
                    ),
                }));
            }
        } catch (e) {
            console.error('[chatStore] Auto-title failed:', e);
        }
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
        set(state => {
            const map = new Map(state.projects.map(p => [p.id, p]));
            const reordered = orderedIds
                .map((id, i) => map.get(id) ? { ...map.get(id)!, sortOrder: i * 1000 } : null)
                .filter(Boolean) as typeof state.projects;
            return { projects: reordered };
        });
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
            set({ activeConversationId: conversationId, activeProjectId: conv?.projectId ?? null, conversationTempChunkIds: [], streamingContent: '', contextStats: { dataCount: 0, patternCount: 0, logCount: 0, patternHints: [], logHints: [] } });
            const raw = await invoke<(Message & { metadata?: string })[]>('get_messages', { conversationId });
            let restoredTempChunkIds: string[] = [];
            const messages: Message[] = raw.map(m => {
                let attachedFiles: Message['attachedFiles'];
                let citationSources: Message['citationSources'];
                let mentionedSources: Message['mentionedSources'];
                let reasoningContent: Message['reasoningContent'];
                try {
                    const meta = JSON.parse(m.metadata || '{}');
                    if (Array.isArray(meta.attachedFiles)) attachedFiles = meta.attachedFiles;
                    if (Array.isArray(meta.citationSources)) citationSources = meta.citationSources;
                    if (Array.isArray(meta.mentionedSources)) mentionedSources = meta.mentionedSources;
                    if (typeof meta.reasoningContent === 'string') reasoningContent = meta.reasoningContent;
                    if (m.role === 'user' && Array.isArray(meta.tempChunkIds)) {
                        restoredTempChunkIds = meta.tempChunkIds;
                    }
                } catch { /* ignore */ }
                return { ...m, attachedFiles, mentionedSources, citationSources, reasoningContent };
            });
            set({ messages, conversationTempChunkIds: restoredTempChunkIds });
        } catch (error) {
            console.error('Failed to load messages:', error);
        }
    },

    triggerUrgentReminderCheck: async (conversationId: string, userMsg: string, aiMsg: string) => {
        const URGENT_PATTERNS = [
            /\d+\s*\u5206\u9418[\u5f8c\u540e].{0,6}(\u958b|\u4f1a|\u6703|\u898b|\u63d0\u9192|deadline)/,
            /\d+\s*\u5c0f\u6642[\u5f8c\u540e].{0,6}(\u958b|\u4f1a|\u6703|\u898b|\u63d0\u9192|deadline)/,
            /[\u4eca\u660e][\u5929\u665a\u65e9].{0,10}(\u958b\u6703|\u6703\u8b70|\u9762\u8a66|\u898b\u9762|\u7d04|\u63d0\u9192|\u622a\u6b62|deadline)/,
            /\u5f8c\u5929.{0,10}(\u958b\u6703|\u6703\u8b70|\u9762\u8a66|\u898b\u9762|\u7d04|\u63d0\u9192|\u622a\u6b62)/,
            /\u5f85[\u6703\u4f1a].{0,6}(\u958b\u6703|\u6703\u8b70|\u9762\u8a66)/,
            /[\u5348\u65e9\u665a]\s*\d+\s*[\u9ede\u70b9].{0,10}(\u958b\u6703|\u6703\u8b70|\u63d0\u9192)/,
            /\d+\s*[\u9ede\u70b9].{0,10}(\u958b\u6703|\u6703\u8b70|\u63d0\u9192)/,
            /[\u4eca\u660e\u5f8c][\u5929]\u622a\u6b62/,
            /\u622a\u6b62[\u65e5\u671f\u6642\u9593].{0,6}(\u662f|\u70ba|\u5728)/,
            /(meeting|call|interview)\s+(in|at)\s+\d/i,
            /remind\s+me\s+(in|at)\s+\d/i,
            /due\s+(today|tomorrow|on)\b/i,
            /deadline\s+(today|tomorrow|on|is)\b/i,
        ];
        const isUrgent = URGENT_PATTERNS.some(p => p.test(userMsg)) || userMsg.includes('\u63d0\u9192');
        console.log('[UrgentReminder] Detection check:', { userMsg, isUrgent });

        if (!isUrgent) return;

        const recentMessages = `User: ${userMsg}\nAssistant: ${aiMsg}`;
        try {
            const ids = await invoke<string[]>('trigger_urgent_reminder_check', {
                conversationId,
                recentMessages,
            });
            if (ids && ids.length > 0) {
                set(state => ({
                    pendingReminderAckByConversation: {
                        ...state.pendingReminderAckByConversation,
                        [conversationId]: true,
                    },
                }));
                toast.success(i18n.t('chat.reminder_created_count', { count: ids.length }));
            }
        } catch (e) {
            console.error('[UrgentReminder] Failed:', e);
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

        const maybeShowReminderToastByAnswer = (answer: string) => {
            const REMINDER_ACK_PATTERNS = [
                /\u63d0\u9192\u5df2\u6210\u529f\u65b0\u589e\u81f3\u60a8\u7684\u884c\u7a0b/,
                /\u5df2\u70ba\u60a8\u8a2d\u5b9a\u63d0\u9192/,
                /\u5df2\u8a18\u9304\u60a8\u7684\u63d0\u9192/,
                /\u5df2\u7eb3\u5165\u6d3b\u52a8\u63d0\u9192/,
                /\u5df2\u7d0d\u5165\u6d3b\u52d5\u63d0\u9192/,
                /\u8a2d\u5b9a\u4e86\u63d0\u9192/,
                /\u8a2d\u5b9a\u6703\u8b70\u63d0\u9192/,
            ];
            if (REMINDER_ACK_PATTERNS.some((p) => p.test(answer))) {
                toast.success(i18n.t('chat.reminder_added_to_schedule'));
            }
        };

        const maybeAppendReminderAck = async (answer: string): Promise<string> => {
            const pending = get().pendingReminderAckByConversation[activeConversationId];
            if (!pending) return answer;
            try {
                const ack = await invoke<string | null>('decide_reminder_ack', {
                    userMessage: content,
                    recentAssistantContext: recentAssistantContext(messages),
                    currentAnswer: answer,
                });
                const shouldAppend = !!(ack && ack.trim());
                set(state => ({
                    pendingReminderAckByConversation: {
                        ...state.pendingReminderAckByConversation,
                        [activeConversationId]: false,
                    },
                }));
                if (shouldAppend) return `${answer.trimEnd()}\n${ack!.trim()}`;
            } catch (e) {
                console.warn('[ReminderAck] decide_reminder_ack failed:', e);
            }
            return answer;
        };

        set({ isGenerating: true, streamingContent: '' });
        try {
            const newChunkIds = opts?.tempChunkIds ?? [];
            const allTempChunkIds = [...get().conversationTempChunkIds, ...newChunkIds];
            if (newChunkIds.length > 0) {
                set({ conversationTempChunkIds: allTempChunkIds });
            }

            const userMsg: Message = {
                id: 'optimistic-user-' + Date.now(),
                role: 'user',
                content,
                created_at: new Date().toISOString(),
                attachedFiles: opts?.attachedFiles,
                mentionedSources: opts?.mentionedSources?.length ? opts.mentionedSources : undefined,
            };
            set(state => ({ messages: [...state.messages, userMsg] }));

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

            const history: [string, string][] = messages
                .filter(m => m.role === 'user' || m.role === 'assistant')
                .slice(-12)
                .map(m => [m.role, m.content]);

            const activeConv = get().conversations.find(c => c.id === activeConversationId);
            const conversationSummary = activeConv?.summary?.trim() || null;

            const ragEnabled = opts?.ragEnabled ?? true;
            const placeholderMsgId = 'streaming-assistant-' + Date.now();

            set(state => ({
                messages: [...state.messages, {
                    id: placeholderMsgId,
                    role: 'assistant' as const,
                    content: '',
                    created_at: new Date().toISOString(),
                }]
            }));

            let accumulated = '';
            let accumulatedReasoning = '';

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

            const unlistenReasoning = await listen<{ conversationId: string; token: string }>(
                'rag-stream-reasoning',
                (event) => {
                    if (event.payload.conversationId !== activeConversationId) return;
                    accumulatedReasoning += event.payload.token;
                    set(state => ({
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, reasoningContent: accumulatedReasoning } : m
                        ),
                    }));
                }
            );

            const unlistenDone = await listen<{ conversationId: string; fullAnswer: string; reasoning?: string | null; citationSources?: string[]; contextHints?: { patternCount?: number; logCount?: number; dataCount?: number; patternHints?: string[]; logHints?: string[] } }>(
                'rag-stream-done',
                async (event) => {
                    if (event.payload.conversationId !== activeConversationId) return;
                    let finalAnswer = event.payload.fullAnswer || accumulated;
                    const finalReasoning = event.payload.reasoning || accumulatedReasoning || undefined;
                    const citationSources = Array.isArray(event.payload.citationSources) ? event.payload.citationSources : [];
                    const hints = event.payload.contextHints;
                    finalAnswer = await maybeAppendReminderAck(finalAnswer);
                    maybeShowReminderToastByAnswer(finalAnswer);

                    set(state => ({
                        isGenerating: false,
                        streamingContent: '',
                        contextStats: hints ? {
                            patternCount: hints.patternCount ?? 0,
                            logCount: hints.logCount ?? 0,
                            dataCount: hints.dataCount ?? 0,
                            patternHints: hints.patternHints ?? [],
                            logHints: hints.logHints ?? [],
                        } : state.contextStats,
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, id: placeholderMsgId.replace('streaming-', ''), content: finalAnswer, citationSources, reasoningContent: finalReasoning } : m
                        ),
                    }));

                    const metaObj: Record<string, unknown> = { citationSources };
                    if (finalReasoning) metaObj.reasoningContent = finalReasoning;

                    invoke('add_message', {
                        conversationId: activeConversationId,
                        role: 'assistant',
                        content: finalAnswer,
                        metadata: JSON.stringify(metaObj),
                    }).then(() => {
                        const msgCount = get().messages.filter(m => m.role === 'user' || m.role === 'assistant').length;
                        if (msgCount <= 4) {
                            get().autoTitleConversation(activeConversationId!);
                        }
                    }).catch(e => console.error('Failed to save assistant message:', e));

                    get().triggerUrgentReminderCheck(activeConversationId!, content, finalAnswer);

                    unlistenToken();
                    unlistenReasoning();
                    unlistenDone();
                }
            );

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
                unlistenReasoning();
                unlistenDone();

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
                    let fallbackAnswer = ragResponse.answer || '(No response)';
                    fallbackAnswer = await maybeAppendReminderAck(fallbackAnswer);
                    maybeShowReminderToastByAnswer(fallbackAnswer);
                    const fallbackCitations = Array.isArray(ragResponse.citationSources) ? ragResponse.citationSources : [];
                    const fbHints = ragResponse.contextHints;
                    await invoke('add_message', {
                        conversationId: activeConversationId,
                        role: 'assistant',
                        content: fallbackAnswer,
                        metadata: JSON.stringify({ citationSources: fallbackCitations }),
                    });
                    set(state => ({
                        isGenerating: false,
                        streamingContent: '',
                        contextStats: fbHints ? {
                            patternCount: fbHints.patternCount ?? 0,
                            logCount: fbHints.logCount ?? 0,
                            dataCount: fbHints.dataCount ?? 0,
                            patternHints: fbHints.patternHints ?? [],
                            logHints: fbHints.logHints ?? [],
                        } : state.contextStats,
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, id: placeholderMsgId.replace('streaming-', ''), content: fallbackAnswer, citationSources: fallbackCitations } : m
                        ),
                    }));
                } catch (fallbackErr) {
                    const errMsg = `(Response failed: ${fallbackErr})`;
                    set(state => ({
                        isGenerating: false,
                        streamingContent: '',
                        messages: state.messages.map(m =>
                            m.id === placeholderMsgId ? { ...m, id: placeholderMsgId.replace('streaming-', ''), content: errMsg } : m
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
