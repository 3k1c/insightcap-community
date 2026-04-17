import React, { useEffect, useRef, useState } from 'react';
import { FileText, Link, AtSign, Copy, Check, Brain, ChevronDown, Loader2, CalendarClock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Message } from '../../stores/chatStore';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
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

const ThinkingBlock = ({ content, isStreaming }: { content: string; isStreaming?: boolean }) => {
    const { t } = useTranslation();
    const [isExpanded, setIsExpanded] = useState(false);
    const prevStreamingRef = useRef(isStreaming);

    // streaming 開始時自動展開，結束時自動收合
    useEffect(() => {
        if (isStreaming && !prevStreamingRef.current) {
            setIsExpanded(true);
        } else if (!isStreaming && prevStreamingRef.current) {
            setIsExpanded(false);
        }
        prevStreamingRef.current = isStreaming;
    }, [isStreaming]);

    return (
        <div className="mb-3 rounded-xl border border-stroke-divider bg-surface-soft overflow-hidden">
            <button
                onClick={() => setIsExpanded(!isExpanded)}
                className="w-full flex items-center gap-2 px-3 py-2 text-fs-sm text-text-secondary hover:bg-surface-base transition-colors"
            >
                <Brain className="w-3.5 h-3.5 flex-shrink-0" />
                <span>{t('chat.thinking_process')}</span>
                {isStreaming && <Loader2 className="w-3 h-3 animate-spin" />}
                <ChevronDown className={`w-3.5 h-3.5 ml-auto transition-transform ${isExpanded ? 'rotate-180' : ''}`} />
            </button>
            {isExpanded && (
                <div className="px-3 py-2 border-t border-stroke-divider text-fs-sm text-text-secondary leading-relaxed max-h-[300px] overflow-y-auto">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {content}
                    </ReactMarkdown>
                    {isStreaming && (
                        <span className="inline-block w-1.5 h-3 bg-text-secondary ml-0.5 animate-pulse align-middle rounded-sm" />
                    )}
                </div>
            )}
        </div>
    );
};

const getDisplayName = (source: string): string => {
    try {
        const hostname = new URL(source).hostname;
        return hostname.replace(/^www\./, '');
    } catch {
        return source;
    }
};

const isUrl = (s: string): boolean => {
    try { return ['http:', 'https:'].includes(new URL(s).protocol); } catch { return false; }
};

const CitationBadge = ({ source }: { source: string }) => {
    const [preview, setPreview] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);
    const sourceIsUrl = isUrl(source);

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
        if (sourceIsUrl) {
            openUrl(source).catch(console.error);
            return;
        }

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
                {getDisplayName(source)}
            </button>
            {!sourceIsUrl && isOpen && (
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
    const userScrolledRef = useRef(false);
    const msgDragInfo = useRef({ isMouseDown: false, isDragging: false, startX: 0, startY: 0, content: '', mouseDownAt: 0 });

    // ── Scroll: 追蹤使用者是否手動 scroll ──────────────────────────────────────
    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        const handleScroll = () => {
            const distFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
            // 距底部超過 80px，視為使用者主動向上捲
            userScrolledRef.current = distFromBottom > 80;
        };

        container.addEventListener('scroll', handleScroll, { passive: true });
        return () => container.removeEventListener('scroll', handleScroll);
    }, []);

    // ── Message drag to editor ────────────────────────────────────────────────
    useEffect(() => {
        const handleMouseMove = (e: globalThis.MouseEvent) => {
            if (!msgDragInfo.current.isMouseDown) return;
            const { startX, startY, isDragging } = msgDragInfo.current;
            if (!isDragging) {
                const dx = e.clientX - startX;
                const dy = e.clientY - startY;
                if (Math.hypot(dx, dy) > 12) {
                    msgDragInfo.current.isDragging = true;
                    document.body.style.userSelect = 'none';
                    document.body.style.cursor = 'grabbing';
                }
                return;
            }
            e.preventDefault();
            const el = document.elementFromPoint(e.clientX, e.clientY);
            const overEditor = !!el?.closest('[data-editor-drop]');
            document.body.style.cursor = overEditor ? 'copy' : 'grabbing';
            window.dispatchEvent(new CustomEvent(overEditor ? 'editor-drag-enter' : 'editor-drag-leave'));
            window.dispatchEvent(new CustomEvent('text-drag-preview', {
                detail: { x: e.clientX, y: e.clientY, text: msgDragInfo.current.content }
            }));
        };

        const handleMouseUp = (e: globalThis.MouseEvent) => {
            if (!msgDragInfo.current.isMouseDown) return;
            const { isDragging, content } = msgDragInfo.current;
            msgDragInfo.current.isMouseDown = false;
            msgDragInfo.current.isDragging = false;
            document.body.style.userSelect = '';
            document.body.style.cursor = '';
            if (isDragging) {
                const el = document.elementFromPoint(e.clientX, e.clientY);
                if (el?.closest('[data-editor-drop]')) {
                    window.dispatchEvent(new CustomEvent('drop-to-editor', { detail: { content, x: e.clientX, y: e.clientY } }));
                }
                window.dispatchEvent(new CustomEvent('editor-drag-leave'));
                window.dispatchEvent(new CustomEvent('text-drag-preview-clear'));
            }
        };

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };
    }, []);
    // ─────────────────────────────────────────────────────────────────────────

    // 訊息新增或 streaming 更新時 scroll 到底部
    // 新訊息加入時強制 scroll；streaming 中若使用者未手動 scroll 則跟隨
    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        const messageAdded = messages.length > prevMessagesLengthRef.current;
        prevMessagesLengthRef.current = messages.length;

        if (messageAdded) {
            // 新訊息：重置手動 scroll 狀態，強制跟隨
            userScrolledRef.current = false;
            container.scrollTop = container.scrollHeight;
        } else if (isGenerating && !userScrolledRef.current) {
            // streaming 更新：未手動 scroll 才跟隨（直接設 scrollTop 避免抖動）
            container.scrollTop = container.scrollHeight;
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
                messages.map((msg) => {
                    const isUser = msg.role === 'user';
                    const isReminder = !isUser && (
                        msg.content.includes('已為您設定提醒') ||
                        msg.content.includes('已收到您的更新') ||
                        msg.content.includes('已納入活動提醒') ||
                        msg.content.includes('已更新為') ||
                        msg.content.includes('已成功提取') ||
                        msg.content.includes('已排程') ||
                        msg.content.includes('提醒事項') ||
                        msg.content.includes('記錄為行程資訊') ||
                        msg.content.includes('設定了提醒') ||
                        msg.content.includes('設定會議提醒') ||
                        msg.content.includes('行程資訊')
                    );

                    return (
                        <div
                            key={msg.id}
                            className={`flex w-full ${isUser ? 'justify-end' : 'justify-start'}`}
                        >
                            <div
                                onMouseDown={(e) => {
                                    if (e.button !== 0) return;
                                    const sel = window.getSelection();
                                    const selectedText = sel?.toString().trim() ?? '';
                                    if (!selectedText || !sel || sel.rangeCount === 0) return;
                                    // 判斷滑鼠是否落在 selection 高亮範圍內
                                    const range = sel.getRangeAt(0);
                                    const rects = range.getClientRects();
                                    let insideSelection = false;
                                    for (let i = 0; i < rects.length; i++) {
                                        const r = rects[i];
                                        if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) {
                                            insideSelection = true;
                                            break;
                                        }
                                    }
                                    if (!insideSelection) return;
                                    // 滑鼠在 selection 範圍內，準備可能的 DnD
                                    e.preventDefault(); // 阻止瀏覽器清除 selection
                                    msgDragInfo.current = { isMouseDown: true, isDragging: false, startX: e.clientX, startY: e.clientY, content: selectedText, mouseDownAt: Date.now() };
                                }}
                                onDragStart={(e) => e.preventDefault()}
                                className={`
                                    min-w-0 max-w-[85%] overflow-hidden rounded-2xl px-4 py-3 relative
                                    ${isUser
                                        ? 'bg-accent-default text-white rounded-br-sm'
                                        : isReminder
                                            ? 'ic-reminder-bg border-accent-default/20 border text-text-primary rounded-bl-sm shadow-[0_2px_8px_-2px_rgba(0,0,0,0.06)]'
                                            : 'bg-surface-subtle text-text-primary rounded-bl-sm border border-stroke-divider'
                                    }
                                  `}
                            >
                                {isReminder && (
                                    <div className="absolute top-2 right-2 opacity-30">
                                        <CalendarClock className="w-3.5 h-3.5 text-accent-default" />
                                    </div>
                                )}

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
                                                <div key={i} className={`flex items-center gap-1.5 rounded-xl px-2.5 py-1.5 max-w-[180px] border ${msg.role === 'user' ? 'bg-white/20 border-white/30 text-white' : 'bg-surface-soft border-stroke-divider text-text-primary'}`}>
                                                    {f.fileType === 'url'
                                                        ? <Link className="w-3 h-3 shrink-0" />
                                                        : <FileText className="w-3 h-3 shrink-0" />
                                                    }
                                                    <span className="text-fs-sm truncate">{f.name}</span>
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
                                {msg.role === 'assistant' && msg.reasoningContent && (
                                    <ThinkingBlock
                                        content={msg.reasoningContent}
                                        isStreaming={isGenerating && msg.id.startsWith('streaming-')}
                                    />
                                )}
                                {(() => {
                                    const isStreamingEmpty = isGenerating && msg.role === 'assistant' && msg.id.startsWith('streaming-') && !msg.content;
                                    if (isStreamingEmpty) return null;
                                    const isUser = msg.role === 'user';
                                    return (
                                        <div className="markdown-body text-fs-base leading-relaxed font-medium min-w-0 overflow-x-hidden">
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
                                                    a: ({ node, ...props }) => <a {...props} target="_blank" rel="noopener noreferrer" className={isUser ? 'text-white underline' : 'text-accent-default hover:underline'} />,
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
                                        </div>
                                    );
                                })()}

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

                                {isGenerating && msg.role === 'assistant' && msg.id.startsWith('streaming-') && (
                                    <div className="text-[10px] mt-2 opacity-50 text-left">
                                        <span className="inline-flex items-center gap-0.5">
                                            <span className="w-1 h-1 rounded-full bg-current animate-bounce [animation-delay:0ms]" />
                                            <span className="w-1 h-1 rounded-full bg-current animate-bounce [animation-delay:150ms]" />
                                            <span className="w-1 h-1 rounded-full bg-current animate-bounce [animation-delay:300ms]" />
                                        </span>
                                    </div>
                                )}
                            </div>
                        </div>
                    );
                })
            )}

            <div ref={bottomRef} />
        </div>
    );
};
