import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface Conversation {
    id: string;
    title: string;
    summary: string;
    createdAt: string;
    updatedAt: string;
}

export interface Message {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    createdAt: string;
}

interface ContextStats {
    dataCount: number;
    patternCount: number;
    logCount: number;
}

interface ChatState {
    conversations: Conversation[];
    activeConversationId: string | null;
    messages: Message[];
    isGenerating: boolean;
    contextStats: ContextStats;

    loadConversations: () => Promise<void>;
    createNewConversation: () => Promise<void>;
    loadMessages: (conversationId: string) => Promise<void>;
    sendMessage: (content: string) => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
    conversations: [],
    activeConversationId: null,
    messages: [],
    isGenerating: false,
    contextStats: { dataCount: 0, patternCount: 0, logCount: 0 },

    loadConversations: async () => {
        try {
            const conversations = await invoke<Conversation[]>('get_conversations');
            set({ conversations });

            const { activeConversationId } = get();
            if (!activeConversationId && conversations.length > 0) {
                set({ activeConversationId: conversations[0].id });
                get().loadMessages(conversations[0].id);
            }
        } catch (error) {
            console.error('Failed to load conversations:', error);
        }
    },

    createNewConversation: async () => {
        try {
            const newId = await invoke<string>('create_conversation');
            await get().loadConversations();
            set({ activeConversationId: newId, messages: [] });
        } catch (error) {
            console.error('Failed to create conversation:', error);
        }
    },

    loadMessages: async (conversationId: string) => {
        try {
            const currentId = get().activeConversationId;
            if (currentId && currentId !== conversationId) {
                // Background summary for the old conversation
                invoke('summarize_conversation', { conversationId: currentId }).catch(e => console.warn('Summary failed:', e));
            }
            set({ activeConversationId: conversationId });
            const messages = await invoke<Message[]>('get_messages', { conversationId });
            set({ messages });
        } catch (error) {
            console.error('Failed to load messages:', error);
        }
    },

    sendMessage: async (content: string) => {
        const { activeConversationId } = get();
        if (!activeConversationId) return;

        // Optimistic UI updates could go here in a real app
        set({ isGenerating: true });
        try {
            // 1. Save user message
            await invoke('add_message', {
                conversationId: activeConversationId,
                role: 'user',
                content,
            });

            // 2. Perform RAG (Placeholder for now)
            const ragResponse = await invoke<any>('rag_query', {
                query: content,
                projectId: null,
            });

            // 3. Save assistant message
            await invoke('add_message', {
                conversationId: activeConversationId,
                role: 'assistant',
                content: ragResponse.answer || "Placeholder answer",
            });

            // Update stats
            const capturesCount = ragResponse.context_used?.captures?.length || 0;
            const memoryChunksCount = ragResponse.context_used?.memory_chunks?.length || 0;
            // 未來有了真實 memory_chunks 後可依 knowledge_type 給 pattern/log
            set({
                contextStats: {
                    dataCount: capturesCount + memoryChunksCount, // Simplified for now
                    patternCount: 0,
                    logCount: 0
                }
            });

            // Reload messages to get assigned IDs and timestamps
            await get().loadMessages(activeConversationId);
        } catch (error) {
            console.error('Failed to send message:', error);
        } finally {
            set({ isGenerating: false });
        }
    },
}));
