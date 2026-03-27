import React, { useEffect } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useKnowledgeStore } from '../stores/knowledgeStore';
import { MessageList } from '../components/chat/MessageList';
import { InputArea } from '../components/chat/InputArea';
import { ContextHintBanner } from '../components/memory/ContextHintBanner';
import { MessageSquarePlus, PenLine, X } from 'lucide-react';
import { useUiStore } from '../stores/uiStore';
import { EditorPane } from '../components/editor/EditorPane';

export const ChatPage: React.FC = () => {
    const {
        conversations,
        activeConversationId,
        messages,
        isGenerating,
        contextStats,
        loadConversations,
        createNewConversation,
        loadMessages,
        sendMessage
    } = useChatStore();

    const { loadSources } = useKnowledgeStore();
    const { isEditorOpen, activeEditorFilePath, openEditor, closeEditor } = useUiStore();

    useEffect(() => {
        loadConversations();
        loadSources();
    }, [loadConversations, loadSources]);

    const handleSendMessage = async (content: string) => {
        if (!activeConversationId) {
            await createNewConversation();
        }
        await sendMessage(content);
    };

    // 當沒有對話被選中時的狀態
    const noConversation = !activeConversationId && conversations.length === 0;

    return (
        <div className="flex h-screen w-full bg-[var(--ic-bg-base)] text-[var(--ic-text-primary)]">

            {/* 側邊欄：歷史對話 */}
            <div className="w-64 border-r border-[var(--ic-border)] bg-[var(--ic-bg-surface)] flex flex-col">
                <div className="p-4 border-b border-[var(--ic-border)] flex justify-between items-center">
                    <h2 className="font-semibold text-sm">對話歷史</h2>
                    <button
                        onClick={() => createNewConversation()}
                        className="p-1.5 hover:bg-[var(--ic-bg-subtle)] rounded text-[var(--ic-text-secondary)] transition-colors"
                        title="新對話"
                    >
                        <MessageSquarePlus className="w-4 h-4" />
                    </button>
                </div>
                <div className="flex-1 overflow-y-auto p-2 space-y-1">
                    {conversations.map(conv => (
                        <div
                            key={conv.id}
                            onClick={() => loadMessages(conv.id)}
                            className={`px-3 py-2 text-sm rounded-lg cursor-pointer truncate transition-colors ${activeConversationId === conv.id
                                ? 'bg-[var(--ic-accent-subtle)] text-[var(--ic-accent-primary)] font-medium'
                                : 'hover:bg-[var(--ic-bg-subtle)] text-[var(--ic-text-secondary)]'
                                }`}
                        >
                            {conv.title === 'New Conversation' ? '新對話' : conv.title}
                        </div>
                    ))}
                    {conversations.length === 0 && (
                        <div className="text-xs text-[var(--ic-text-muted)] p-4 text-center">
                            尚無對話記錄
                        </div>
                    )}
                </div>
            </div>

            {/* 主對話區域 */}
            <div className={`flex flex-col h-full bg-[var(--ic-bg-base)] transition-all ${isEditorOpen ? 'w-[400px] border-r border-[var(--ic-border)]' : 'flex-1'}`}>
                {/* Chat Top Bar */}
                <div className="h-12 border-b border-[var(--ic-border)] bg-[var(--ic-bg-surface)] flex items-center justify-between px-4 shrink-0">
                    <div className="font-semibold text-sm truncate pr-4 text-[var(--ic-text-secondary)]">
                        {conversations.find(c => c.id === activeConversationId)?.title || 'InsightCAP 助理'}
                    </div>
                    {!isEditorOpen && (
                        <button
                            onClick={() => openEditor(`notes/draft_${new Date().getTime()}.md`)}
                            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-[var(--ic-accent-primary)] hover:bg-[var(--ic-accent-subtle)] rounded-md transition-colors shadow-sm"
                            title="打開文檔編輯器"
                        >
                            <PenLine className="w-3.5 h-3.5" /> 在旁邊寫作
                        </button>
                    )}
                </div>

                {noConversation ? (
                    <div className="flex-1 flex flex-col items-center justify-center">
                        <h1 className="text-2xl font-bold mb-4">InsightCAP 即時對話</h1>
                        <p className="text-[var(--ic-text-muted)] mb-8">開始詢問關於您的經驗與知識的問題</p>
                        <button
                            onClick={() => createNewConversation()}
                            className="px-6 py-2 bg-[var(--ic-accent-primary)] text-white rounded-lg hover:bg-[var(--ic-accent-hover)] transition-colors shadow-sm"
                        >
                            開始新對話
                        </button>
                    </div>
                ) : (
                    <>
                        <ContextHintBanner stats={contextStats} isInjecting={isGenerating} />
                        <MessageList messages={messages} isGenerating={isGenerating} />
                        <InputArea onSendMessage={handleSendMessage} isGenerating={isGenerating} />
                    </>
                )}
            </div>

            {/* 右側編輯器區域 */}
            {isEditorOpen && activeEditorFilePath && (
                <div className="flex-1 flex flex-col h-full bg-[var(--ic-bg-surface)] overflow-hidden">
                    <div className="h-12 border-b border-[var(--ic-border)] bg-[var(--ic-bg-elevated)] flex items-center justify-between px-4 shrink-0">
                        <div className="flex bg-[var(--ic-bg-base)] border border-[var(--ic-border)] rounded-md px-2 py-1 items-center gap-2 max-w-sm overflow-hidden">
                            <span className="text-xs text-[var(--ic-text-muted)] truncate">{activeEditorFilePath}</span>
                        </div>
                        <button
                            onClick={closeEditor}
                            className="p-1 hover:bg-[var(--ic-bg-hover)] text-[var(--ic-text-secondary)] rounded transition-colors"
                        >
                            <X className="w-4 h-4" />
                        </button>
                    </div>
                    <div className="flex-1 overflow-hidden p-2">
                        <EditorPane filePath={activeEditorFilePath} />
                    </div>
                </div>
            )}

        </div>
    );
};
