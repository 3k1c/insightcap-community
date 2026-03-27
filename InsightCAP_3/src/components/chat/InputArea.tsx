import React, { useState, KeyboardEvent, useRef, useEffect } from 'react';
import { Send, Hash, FileText } from 'lucide-react';
import { useTagStore } from '../../stores/tagStore';
import { useKnowledgeStore } from '../../stores/knowledgeStore';

interface InputAreaProps {
    onSendMessage: (content: string) => void;
    isGenerating?: boolean;
}

export const InputArea: React.FC<InputAreaProps> = ({
    onSendMessage,
    isGenerating
}) => {
    const [input, setInput] = useState('');
    const [showTags, setShowTags] = useState(false);
    const [showSources, setShowSources] = useState(false);
    const [sourceQuery, setSourceQuery] = useState('');
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const { suggestions, fetchSuggestions, recentTags } = useTagStore();
    const { sources, loadSources } = useKnowledgeStore();

    useEffect(() => {
        // Tag handling
        if (input.endsWith('#')) {
            setShowTags(true);
            setShowSources(false);
            fetchSuggestions('');
        } else if (showTags) {
            const match = input.match(/#([^ \n]*)$/);
            if (match) {
                fetchSuggestions(match[1]);
            } else {
                setShowTags(false);
            }
        }

        // Source handling
        if (input.endsWith('@')) {
            setShowSources(true);
            setShowTags(false);
            setSourceQuery('');
            if (sources.length === 0) loadSources();
        } else if (showSources) {
            const match = input.match(/@([^ \n]*)$/);
            if (match) {
                setSourceQuery(match[1]);
            } else {
                setShowSources(false);
            }
        }
    }, [input, showTags, showSources, fetchSuggestions, sources.length, loadSources]);

    const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const handleSend = () => {
        if (!input.trim() || isGenerating) return;
        onSendMessage(input.trim());
        setInput('');
        setShowTags(false);

        // Reset textarea height
        if (textareaRef.current) {
            textareaRef.current.style.height = 'auto';
        }
    };

    const insertTag = (tag: string) => {
        const newVal = input.replace(/#[^ \n]*$/, `#${tag} `);
        setInput(newVal);
        setShowTags(false);
        textareaRef.current?.focus();
    };

    const insertSource = (title: string) => {
        const newVal = input.replace(/@[^ \n]*$/, `@${title} `);
        setInput(newVal);
        setShowSources(false);
        textareaRef.current?.focus();
    };

    const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
        setInput(e.target.value);
        e.target.style.height = 'auto';
        e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
    };

    return (
        <div className="relative border-t border-[var(--ic-border)] bg-[var(--ic-bg-card)] px-4 py-3">
            {/* Tag Suggestions Popup */}
            {showTags && (suggestions.length > 0 || recentTags.length > 0) && (
                <div className="absolute bottom-full left-4 mb-2 w-64 bg-[var(--ic-bg-card)] border border-[var(--ic-border)] rounded-lg shadow-lg overflow-hidden animate-in slide-in-from-bottom-2 z-50">
                    <div className="px-3 py-2 border-b border-[var(--ic-border)] bg-[var(--ic-bg-subtle)] text-xs font-semibold text-[var(--ic-text-muted)] flex items-center gap-1.5">
                        <Hash className="w-3 h-3" />
                        建議標籤
                    </div>
                    <ul className="max-h-48 overflow-y-auto py-1">
                        {(suggestions.length > 0 ? suggestions : recentTags.map(t => t.name)).map((tag, idx) => (
                            <li
                                key={idx}
                                className="px-3 py-2 text-sm hover:bg-[var(--ic-bg-subtle)] cursor-pointer text-[var(--ic-text-primary)] transition-colors"
                                onClick={() => insertTag(tag)}
                            >
                                #{tag}
                            </li>
                        ))}
                    </ul>
                </div>
            )}

            {/* Source Suggestions Popup */}
            {showSources && sources.length > 0 && (
                <div className="absolute bottom-full left-4 mb-2 w-72 bg-[var(--ic-bg-card)] border border-[var(--ic-border)] rounded-lg shadow-lg overflow-hidden animate-in slide-in-from-bottom-2 z-50">
                    <div className="px-3 py-2 border-b border-[var(--ic-border)] bg-[var(--ic-bg-subtle)] text-xs font-semibold text-[var(--ic-text-muted)] flex items-center gap-1.5">
                        <FileText className="w-3 h-3" />
                        引用文件來源
                    </div>
                    <ul className="max-h-48 overflow-y-auto py-1">
                        {sources.filter(s => s.title.toLowerCase().includes(sourceQuery.toLowerCase())).map((source) => (
                            <li
                                key={source.id}
                                className="px-3 py-2 text-sm hover:bg-[var(--ic-bg-subtle)] cursor-pointer text-[var(--ic-text-primary)] transition-colors truncate"
                                onClick={() => insertSource(source.title)}
                                title={source.title}
                            >
                                @{source.title}
                            </li>
                        ))}
                    </ul>
                </div>
            )}

            <div className="relative flex items-end gap-2 bg-[var(--ic-bg-subtle)] rounded-xl border border-[var(--ic-border)] focus-within:ring-2 focus-within:ring-[var(--ic-accent-primary)]/20 focus-within:border-[var(--ic-accent-primary)] transition-all p-1">
                <textarea
                    ref={textareaRef}
                    value={input}
                    onChange={handleInput}
                    onKeyDown={handleKeyDown}
                    placeholder="詢問任何問題，輸入 # 加入標籤過濾..."
                    className="flex-1 max-h-[200px] min-h-[40px] bg-transparent border-0 focus:ring-0 resize-none py-2.5 px-3 text-sm text-[var(--ic-text-primary)] placeholder:text-[var(--ic-text-muted)]"
                    rows={1}
                />
                <button
                    onClick={handleSend}
                    disabled={!input.trim() || isGenerating}
                    className="mr-1 mb-1 p-2 rounded-lg bg-[var(--ic-accent-primary)] text-white disabled:opacity-50 disabled:bg-[var(--ic-bg-subtle)] disabled:text-[var(--ic-text-muted)] hover:bg-[var(--ic-accent-primary)]/90 transition-colors"
                >
                    <Send className="w-4 h-4" />
                </button>
            </div>
        </div>
    );
};
