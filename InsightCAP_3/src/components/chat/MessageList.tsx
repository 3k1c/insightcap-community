import React from 'react';
import { Message } from '../../stores/chatStore';

interface MessageListProps {
    messages: Message[];
    isGenerating?: boolean;
}

export const MessageList: React.FC<MessageListProps> = ({
    messages,
    isGenerating
}) => {

    return (
        <div className="flex-1 overflow-y-auto px-4 py-6 space-y-6">
            {messages.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-[var(--ic-text-muted)] opacity-70">
                    <div className="w-16 h-16 rounded-full bg-[var(--ic-bg-subtle)] flex items-center justify-center mb-4">
                        <span className="text-2xl">✨</span>
                    </div>
                    <p>新對話已建立</p>
                    <p className="text-sm mt-1">您可以詢問關於收藏的資料，或者直接進行對話。</p>
                </div>
            ) : (
                messages.map((msg) => (
                    <div
                        key={msg.id}
                        className={`flex w-full ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
                    >
                        <div
                            className={`
                max-w-[85%] rounded-2xl px-4 py-3
                ${msg.role === 'user'
                                    ? 'bg-[var(--ic-accent-primary)] text-white rounded-br-sm'
                                    : 'bg-[var(--ic-bg-subtle)] text-[var(--ic-text-primary)] rounded-bl-sm border border-[var(--ic-border)]'
                                }
              `}
                        >
                            <div className="whitespace-pre-wrap text-sm leading-relaxed font-medium">
                                {msg.content}
                            </div>

                            <div className={`text-[10px] mt-2 opacity-50 ${msg.role === 'user' ? 'text-right text-white' : 'text-left'}`}>
                                {new Date(msg.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                            </div>
                        </div>
                    </div>
                ))
            )}

            {isGenerating && (
                <div className="flex w-full justify-start">
                    <div className="max-w-[85%] rounded-2xl px-4 py-3 bg-[var(--ic-bg-subtle)] text-[var(--ic-text-primary)] rounded-bl-sm border border-[var(--ic-border)] flex items-center gap-2">
                        <div className="flex space-x-1">
                            <div className="w-2 h-2 rounded-full bg-[var(--ic-text-muted)] animate-bounce" style={{ animationDelay: '0ms' }} />
                            <div className="w-2 h-2 rounded-full bg-[var(--ic-text-muted)] animate-bounce" style={{ animationDelay: '150ms' }} />
                            <div className="w-2 h-2 rounded-full bg-[var(--ic-text-muted)] animate-bounce" style={{ animationDelay: '300ms' }} />
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
