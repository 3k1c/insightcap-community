import React, { useEffect, useRef, useState } from 'react';
import { FileText, Link, AtSign, Copy, Check } from 'lucide-react';
import { Message } from '../../stores/chatStore';
import { invoke } from '@tauri-apps/api/core';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';

const CodeBlock = ({ language, value }: { language: string; value: string }) => {
    const [copied, setCopied] = useState(false);

    const handleCopy = () => {
        navigator.clipboard.writeText(value);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    return (
        <div className="relative group my-3 rounded-xl overflow-hidden border border-stroke-divider bg-[#282c34]">
            <div className="flex items-center justify-between px-4 py-2 bg-white/5 border-b border-white/10">
                <span className="text-[10px] font-mono text-text-tertiary uppercase tracking-wider">{language || 'code'}</span>
                <button
                    onClick={handleCopy}
                    className="flex items-center gap-1.5 text-text-tertiary hover:text-white transition-colors"
                >
                    {copied ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5" />}
                    <span className="text-[10px] font-medium">{copied ? '已複製' : '加密'}</span>
                </button>
            </div>
            <SyntaxHighlighter
                language={language}
                style={oneDark}
                customStyle={{
                    margin: 0,
                    padding: '1rem',
                    fontSize: '13px',
                    background: 'transparent',
                }}
            >
                {value}
            </SyntaxHighlighter>
        </div>
    );
};

const CitationBadge = ({ source }: { source: string }) => {
    const [preview, setPreview] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        if (isOpen) {
            document.addEventListener('mousedown', handleClickOutside);
        }
        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
        };
    }, [isOpen]);

    const handleClick = async () => {
        const nextOpen = !isOpen;
        setIsOpen(nextOpen);
        if (!nextOpen || preview || loading) return;

        setLoading(true);
        try {
            const res = await invoke<string>('get_source_preview_by_title', { title: source });
            setPreview(res);
        } catch (e) {
            console.error('Failed to get source preview', e);
            setPreview('無法載入預覽');
        } finally {
            setLoading(false);
        }
    };

    return (
        <div
            ref={containerRef}
            className="flex items-center"
        >
            <button
                onClick={handleClick}
                className={`inline-flex items-center rounded-md border border-stroke-divider px-2 py-0.5 text-[11.5px] opacity-90 transition-all ${isOpen ? 'bg-surface-base border-accent-default/40 text-accent-default shadow-sm' : 'bg-surface-soft hover:bg-surface-base'
                    }`}
            >
                {source}
            </button>
            {isOpen && (
                <div className="absolute bottom-[calc(100%+8px)] left-0 w-full min-w-[300px] max-h-[200px] flex flex-col p-3 bg-surface-flyout border border-stroke-divider rounded-xl shadow-2xl z-[100] origin-bottom-left shadow-black/20">
                    <div className="flex justify-between items-center mb-2 pb-2 border-b border-stroke-divider/40 shrink-0">
                        <div className="text-fs-xs font-semibold text-text-secondary tracking-wide">來源預覽</div>
                        <button onClick={() => setIsOpen(false)} className="text-text-tertiary hover:text-text-primary text-[10px] uppercase font-medium tracking-wide">關閉</button>
                    </div>
                    <div className="overflow-y-auto text-text-primary text-[13px] leading-relaxed select-text pr-1">
                        {loading ? <div className="py-4 text-center text-text-tertiary">載入中...</div> : (preview || '查無預覽內容')}
                    </div>
                </div>
            )}
        </div>
    );
};

interface MessageListProps {
    messages: Message[];
    isGenerating?: boolean;
}

export const MessageList: React.FC<MessageListProps> = ({
    messages,
    isGenerating
}) => {
    const bottomRef = useRef<HTMLDivElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const prevMessagesLengthRef = useRef(messages.length);

    // 訊息新增或 streaming 更新時 scroll 到底部
    // 如果新增了訊息或正在生成，或者原本就很接近底部，則強制捲動
    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        const distFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
        const messageAdded = messages.length > prevMessagesLengthRef.current;
        prevMessagesLengthRef.current = messages.length;

        if (distFromBottom < 300 || messageAdded || isGenerating) {
            bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
        }
    }, [messages, isGenerating]);

    return (
        <div ref={containerRef} className="flex-1 overflow-y-auto px-4 py-6 space-y-6">
            {messages.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-text-tertiary opacity-70">
                    <div className="w-16 h-16 rounded-full bg-surface-subtle flex items-center justify-center mb-4">
                        <span className="text-fs-2xl">✨</span>
                    </div>
                    <p>新對話已建立</p>
                    <p className="text-fs-sm mt-1">您可以詢問關於收藏的資料，或者直接進行對話。</p>
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
                                    ? 'bg-accent-default text-white rounded-br-sm'
                                    : 'bg-surface-subtle text-text-primary rounded-bl-sm border border-stroke-divider'
                                }
              `}
                        >
                            {msg.attachedFiles && msg.attachedFiles.length > 0 && (
                                <div className="flex flex-wrap gap-2 mb-2">
                                    {msg.attachedFiles.map((f, i) =>
                                        f.fileType === 'image_ocr' && f.previewUrl ? (
                                            <div key={i} className="relative rounded-xl overflow-hidden w-20 h-20 shrink-0 border border-white/20">
                                                <img src={f.previewUrl} alt={f.name} className="w-full h-full object-cover" />
                                                <div className="absolute inset-x-0 bottom-0 bg-black/50 px-1 py-0.5">
                                                    <span className="text-[9px] text-white font-medium block truncate">OCR</span>
                                                </div>
                                            </div>
                                        ) : (
                                            <div key={i} className="flex items-center gap-1.5 rounded-xl px-2.5 py-1.5 bg-white/15 border border-white/20 max-w-[180px]">
                                                {f.fileType === 'url'
                                                    ? <Link className="w-3 h-3 shrink-0 opacity-80" />
                                                    : <FileText className="w-3 h-3 shrink-0 opacity-80" />
                                                }
                                                <span className="text-fs-sm truncate opacity-90">{f.name}</span>
                                            </div>
                                        )
                                    )}
                                </div>
                            )}
                            {msg.role === 'user' && msg.mentionedSources && msg.mentionedSources.length > 0 && (
                                <div className="flex flex-wrap gap-1.5 mb-2">
                                    {msg.mentionedSources.map(src => (
                                        <span
                                            key={src.id}
                                            className="inline-flex items-center gap-1 rounded-lg px-2 py-0.5 bg-white/15 border border-white/20 text-[11px]"
                                        >
                                            <AtSign className="w-2.5 h-2.5 opacity-80" />
                                            <span className="truncate max-w-[120px] opacity-90">{src.title}</span>
                                        </span>
                                    ))}
                                </div>
                            )}
                            <div className="markdown-body text-fs-base leading-relaxed font-medium">
                                <ReactMarkdown
                                    remarkPlugins={[remarkGfm]}
                                    components={{
                                        code({ node, inline, className, children, ...props }: any) {
                                            const match = /language-(\w+)/.exec(className || '');
                                            return !inline && match ? (
                                                <CodeBlock
                                                    language={match[1]}
                                                    value={String(children).replace(/\n$/, '')}
                                                />
                                            ) : (
                                                <code className={`${className} bg-black/10 rounded px-1 py-0.5 font-mono text-[0.9em]`} {...props}>
                                                    {children}
                                                </code>
                                            );
                                        },
                                        // 讓連結在新視窗開啟
                                        a: ({ node, ...props }) => <a {...props} target="_blank" rel="noopener noreferrer" className="text-accent-default hover:underline" />,
                                        p: ({ node, ...props }) => <p {...props} className="mb-3 last:mb-0" />,
                                        ul: ({ node, ...props }) => <ul {...props} className="list-disc pl-5 mb-3" />,
                                        ol: ({ node, ...props }) => <ol {...props} className="list-decimal pl-5 mb-3" />,
                                        li: ({ node, ...props }) => <li {...props} className="mb-1" />,
                                        h1: ({ node, ...props }) => <h1 {...props} className="text-xl font-bold mb-3 mt-4" />,
                                        h2: ({ node, ...props }) => <h2 {...props} className="text-lg font-bold mb-2 mt-3" />,
                                        table: ({ node, ...props }) => (
                                            <div className="overflow-x-auto my-3 border border-stroke-divider rounded-lg">
                                                <table {...props} className="w-full border-collapse text-fs-sm" />
                                            </div>
                                        ),
                                        th: ({ node, ...props }) => <th {...props} className="bg-surface-soft p-2 border border-stroke-divider font-semibold text-left" />,
                                        td: ({ node, ...props }) => <td {...props} className="p-2 border border-stroke-divider" />,
                                        blockquote: ({ node, ...props }) => <blockquote {...props} className="border-l-4 border-stroke-divider pl-4 italic opacity-80 my-3" />,
                                    }}
                                >
                                    {msg.content}
                                </ReactMarkdown>
                                {/* streaming 游標：assistant 且正在生成中才顯示 */}
                                {isGenerating && msg.role === 'assistant' && msg.id.startsWith('streaming-') && (
                                    <span className="inline-block w-1.5 h-4 bg-accent-default ml-1 animate-pulse align-middle rounded-sm" />
                                )}
                            </div>

                            {msg.role === 'assistant' && msg.citationSources && msg.citationSources.length > 0 && (
                                <div className="mt-2">
                                    <div className="text-[10px] uppercase tracking-wide opacity-60 mb-1">引用來源</div>
                                    <div className="relative w-full flex flex-wrap gap-1.5">
                                        {msg.citationSources.map((source, idx) => (
                                            <CitationBadge key={`${msg.id}-src-${idx}`} source={source} />
                                        ))}
                                    </div>
                                </div>
                            )}

                            <div className={`text-[10px] mt-2 opacity-50 ${msg.role === 'user' ? 'text-right text-white' : 'text-left'}`}>
                                {msg.created_at ? new Date(msg.created_at.replace(' ', 'T')).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : ''}
                            </div>
                        </div>
                    </div>
                ))
            )}

            <div ref={bottomRef} />
        </div>
    );
};
