import React, { useState, KeyboardEvent, useRef, useEffect, useCallback, useMemo } from 'react';
import { Send, Database, Globe, X, AtSign, FileText, Link, Quote, Loader2, Hash, FileImage, Plus, Brain } from 'lucide-react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useKnowledgeStore } from '../../stores/knowledgeStore';
import { useTagStore } from '../../stores/tagStore';
import { useChatStore } from '../../stores/chatStore';
import { useT } from '../../hooks/useT';

interface AttachedFile {
    id: string;
    name: string;
    filePath: string;
    fileType: 'document' | 'image_ocr' | 'url';
    icon: React.ReactNode;
    previewUrl?: string;
    tempChunkIds?: string[];
    isParsing?: boolean;
    isError?: boolean;
}

interface MentionSource {
    id: string;
    title: string;
    source_type?: string;
}

interface MentionTag {
    id: string;
    name: string;
    useCount?: number;
}

interface QuotedMessage {
    id: string;
    content: string;
}

export interface InputAreaProps {
    onSendMessage: (
        content: string,
        opts?: {
            ragEnabled: boolean;
            webEnabled: boolean;
            mentionedSourceIds: string[];
            mentionedSources: { id: string; title: string }[];
            mentionedTagNames: string[];
            mentionedTags: { id: string; name: string }[];
            attachedFiles: { name: string; filePath: string; fileType: string; previewUrl?: string; tempChunkIds?: string[] }[];
            tempChunkIds?: string[];
            thinkingMode?: 'normal' | 'think';
        }
    ) => void;
    isGenerating?: boolean;
}

const FILE_MENU_ITEMS = [
    {
        key: 'document' as const,
        labelKey: 'chat.add_document',
        icon: <FileText className="w-3 h-3" />,
        accept: '.txt,.md,.doc,.docx,.xlsx,.csv,.pptx,.pdf,.py,.js,.ts,.jsx,.tsx,.swift,.rs,.go,.java,.cpp,.c,.h,.rb,.php,.html,.wav,.mp3,.m4a,.aac,.flac,.ogg,.opus,.webm',
        multiple: true,
    },
    {
        key: 'image_ocr' as const,
        labelKey: 'chat.add_image_ocr',
        icon: <FileImage className="w-3 h-3" />,
        accept: '.png,.jpg,.jpeg,.webp,.gif',
        multiple: true,
    },
    {
        key: 'url' as const,
        labelKey: 'chat.add_url',
        icon: <Link className="w-3 h-3" />,
        accept: null,
        multiple: false,
    },
] as const;

type FileMenuKey = typeof FILE_MENU_ITEMS[number]['key'];


export const InputArea: React.FC<InputAreaProps> = ({ onSendMessage, isGenerating }) => {

    const [input, setInput] = useState('');
    const [ragEnabled, setRagEnabled] = useState(false);
    const [webEnabled, setWebEnabled] = useState(false);
    const [thinkingMode, setThinkingMode] = useState<'normal' | 'think'>('normal');
    const [supportsThinking, setSupportsThinking] = useState(false);

    const [attachedFiles, setAttachedFiles] = useState<AttachedFile[]>([]);
    const [quotedMessage, setQuotedMessage] = useState<QuotedMessage | null>(null);

    const [urlInputOpen, setUrlInputOpen] = useState(false);
    const [urlInputValue, setUrlInputValue] = useState('');

    const [fileMenuOpen, setFileMenuOpen] = useState(false);
    const fileMenuRef = useRef<HTMLDivElement>(null);

    const [mentionedSources, setMentionedSources] = useState<{ id: string; title: string }[]>([]);
    const [isMentionOpen, setIsMentionOpen] = useState(false);
    const [mentionQuery, setMentionQuery] = useState('');
    const [mentionResults, setMentionResults] = useState<MentionSource[]>([]);
    const [mentionIndex, setMentionIndex] = useState(0);
    const [mentionCursorStart, setMentionCursorStart] = useState(-1);
    const mentionDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const [mentionedTags, setMentionedTags] = useState<{ id: string; name: string }[]>([]);
    const [isTagOpen, setIsTagOpen] = useState(false);
    const [tagQuery, setTagQuery] = useState('');
    const [tagResults, setTagResults] = useState<MentionTag[]>([]);
    const [tagIndex, setTagIndex] = useState(0);
    const [tagCursorStart, setTagCursorStart] = useState(-1);
    const tagDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const urlInputRef = useRef<HTMLInputElement>(null);

    const { sources, loadSources } = useKnowledgeStore();
    const { recentTags, loadRecentTags, fetchSuggestions, suggestions } = useTagStore();
    const activeConversationId = useChatStore(s => s.activeConversationId);
    const t = useT();

    useEffect(() => {
        invoke<boolean>('get_chat_llm_supports_thinking')
            .then(ok => setSupportsThinking(ok))
            .catch(() => setSupportsThinking(false));
    }, []);

    useEffect(() => {
        if (ragEnabled && recentTags.length === 0) loadRecentTags();
    }, [ragEnabled, recentTags.length, loadRecentTags]);

    const recommendedTags = useMemo(() => {
        if (!ragEnabled) return [];
        const selectedNames = new Set(mentionedTags.map(t => t.name));
        return recentTags
            .filter(t => !selectedNames.has(t.name))
            .sort((a, b) => (b.recentCount ?? b.useCount) - (a.recentCount ?? a.useCount))
            .slice(0, 5);
    }, [ragEnabled, recentTags, mentionedTags]);

    useEffect(() => {
        const handleQuote = (e: Event) => {
            const detail = (e as CustomEvent<QuotedMessage>).detail;
            setQuotedMessage(detail);
            textareaRef.current?.focus();
        };
        window.addEventListener('quote-message', handleQuote);
        return () => window.removeEventListener('quote-message', handleQuote);
    }, []);

    useEffect(() => {
        const handleClickOutside = (e: MouseEvent) => {
            if (fileMenuRef.current && !fileMenuRef.current.contains(e.target as Node)) {
                setFileMenuOpen(false);
            }
        };
        if (fileMenuOpen) document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [fileMenuOpen]);

    useEffect(() => {
        if (urlInputOpen) setTimeout(() => urlInputRef.current?.focus(), 50);
    }, [urlInputOpen]);

    const searchMentionSources = useCallback((q: string) => {
        const filtered = sources
            .filter(s => s.title.toLowerCase().includes(q.toLowerCase()))
            .slice(0, 8)
            .map(s => ({ id: s.id, title: s.title, source_type: s.type }));
        setMentionResults(filtered);
    }, [sources]);

    const selectMention = useCallback((source: MentionSource) => {
        setMentionedSources(prev =>
            prev.some(s => s.id === source.id) ? prev : [...prev, { id: source.id, title: source.title }]
        );
        if (mentionCursorStart >= 0 && textareaRef.current) {
            const cursor = textareaRef.current.selectionStart;
            const before = input.slice(0, mentionCursorStart);
            const after = input.slice(cursor);
            setInput(before + after);
        }
        setIsMentionOpen(false);
        setMentionQuery('');
        setMentionIndex(0);
        textareaRef.current?.focus();
    }, [input, mentionCursorStart]);

    const searchTags = useCallback((q: string) => {
        if (q.trim() === '') {
            setTagResults(recentTags.slice(0, 8).map(t => ({ id: t.id, name: t.name, useCount: t.useCount })));
        } else {
            const fromRecent = recentTags
                .filter(t => t.name.toLowerCase().includes(q.toLowerCase()))
                .slice(0, 8)
                .map(t => ({ id: t.id, name: t.name, useCount: t.useCount }));
            setTagResults(fromRecent);
            fetchSuggestions(q);
        }
    }, [recentTags, fetchSuggestions]);

    useEffect(() => {
        if (isTagOpen && tagQuery.trim() !== '' && suggestions.length > 0) {
            const fromSuggestions = suggestions.map((name, i) => ({ id: `sug-${i}`, name }));
            setTagResults(prev => {
                const existing = new Set(prev.map(t => t.name));
                const merged = [...prev, ...fromSuggestions.filter(s => !existing.has(s.name))];
                return merged.slice(0, 8);
            });
        }
    }, [suggestions, isTagOpen, tagQuery]);

    const selectTag = useCallback((tag: MentionTag) => {
        setMentionedTags(prev =>
            prev.some(t => t.name === tag.name) ? prev : [...prev, { id: tag.id, name: tag.name }]
        );
        if (tagCursorStart >= 0 && textareaRef.current) {
            const cursor = textareaRef.current.selectionStart;
            const before = input.slice(0, tagCursorStart);
            const after = input.slice(cursor);
            setInput(before + after);
        }
        setIsTagOpen(false);
        setTagQuery('');
        setTagIndex(0);
        textareaRef.current?.focus();
    }, [input, tagCursorStart]);

    const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
        const val = e.target.value;
        const cursor = e.target.selectionStart;
        setInput(val);
        e.target.style.height = 'auto';
        e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;

        const hashMatch = val.slice(0, cursor).match(/#([^#\s]*)$/);
        if (hashMatch) {
            const query = hashMatch[1];
            setTagCursorStart(cursor - query.length - 1);
            setTagQuery(query);
            setTagIndex(0);
            setIsTagOpen(true);
            setIsMentionOpen(false);
            if (tagDebounceRef.current) clearTimeout(tagDebounceRef.current);
            if (recentTags.length === 0) loadRecentTags();
            tagDebounceRef.current = setTimeout(() => searchTags(query), 150);
            return;
        } else if (isTagOpen) {
            setIsTagOpen(false);
        }

        const atMatch = val.slice(0, cursor).match(/@([^@\s]*)$/);
        if (atMatch) {
            const query = atMatch[1];
            setMentionCursorStart(cursor - query.length - 1);
            setMentionQuery(query);
            setMentionIndex(0);
            setIsMentionOpen(true);
            if (mentionDebounceRef.current) clearTimeout(mentionDebounceRef.current);
            if (sources.length === 0) loadSources();
            mentionDebounceRef.current = setTimeout(() => searchMentionSources(query), 150);
        } else if (isMentionOpen) {
            setIsMentionOpen(false);
        }
    };

    const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
        if (isTagOpen && tagResults.length > 0) {
            if (e.key === 'ArrowDown') { e.preventDefault(); setTagIndex(p => (p + 1) % tagResults.length); return; }
            if (e.key === 'ArrowUp') { e.preventDefault(); setTagIndex(p => (p - 1 + tagResults.length) % tagResults.length); return; }
            if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); selectTag(tagResults[tagIndex]); return; }
            if (e.key === 'Escape') { e.preventDefault(); setIsTagOpen(false); return; }
        }
        if (isMentionOpen && mentionResults.length > 0) {
            if (e.key === 'ArrowDown') { e.preventDefault(); setMentionIndex(p => (p + 1) % mentionResults.length); return; }
            if (e.key === 'ArrowUp') { e.preventDefault(); setMentionIndex(p => (p - 1 + mentionResults.length) % mentionResults.length); return; }
            if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); selectMention(mentionResults[mentionIndex]); return; }
            if (e.key === 'Escape') { e.preventDefault(); setIsMentionOpen(false); return; }
        }
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const handleSend = () => {
        const text = input.trim();
        const hasAttachments = attachedFiles.length > 0 || !!quotedMessage || mentionedSources.length > 0 || mentionedTags.length > 0;
        if ((!text && !hasAttachments) || isGenerating) return;

        if (attachedFiles.some(f => f.isParsing)) return;

        let finalContent = text;
        if (quotedMessage) {
            finalContent = `> [link](#quote_${quotedMessage.id}) ${quotedMessage.content.split('\n').join('\n> ')}\n\n${text}`;
        }

        const validFiles = attachedFiles.filter(f => !f.isError);
        const allTempChunkIds = validFiles.flatMap(f => f.tempChunkIds ?? []);
        onSendMessage(finalContent, {
            ragEnabled,
            webEnabled,
            mentionedSourceIds: mentionedSources.map(s => s.id),
            mentionedSources: mentionedSources.map(s => ({ id: s.id, title: s.title })),
            mentionedTagNames: mentionedTags.map(t => t.name),
            mentionedTags: mentionedTags.map(t => ({ id: t.id, name: t.name })),
            attachedFiles: validFiles.map(f => ({ name: f.name, filePath: f.filePath, fileType: f.fileType, previewUrl: f.previewUrl, tempChunkIds: f.tempChunkIds })),
            tempChunkIds: allTempChunkIds.length > 0 ? allTempChunkIds : undefined,
            thinkingMode,
        });

        setInput('');
        setQuotedMessage(null);
        setMentionedSources([]);
        setMentionedTags([]);
        setAttachedFiles([]);
        setIsMentionOpen(false);
        setIsTagOpen(false);
        if (textareaRef.current) textareaRef.current.style.height = 'auto';
    };

    const parseTempFile = async (fileId: string, filePath: string | null, url: string | null) => {
        if (!activeConversationId) return;
        try {
            const ids = await invoke<string[]>('create_temp_chunk', {
                filePath,
                url,
                conversationId: activeConversationId,
            });
            setAttachedFiles(prev => prev.map(f =>
                f.id === fileId ? { ...f, isParsing: false, tempChunkIds: ids } : f
            ));
        } catch (err) {
            console.error('[InputArea] create_temp_chunk error:', err);
            setAttachedFiles(prev => prev.map(f =>
                f.id === fileId ? { ...f, isParsing: false, isError: true } : f
            ));
        }
    };

    const handlePickFile = async (item: typeof FILE_MENU_ITEMS[number]) => {
        setFileMenuOpen(false);

        if (item.key === 'url') {
            setUrlInputOpen(true);
            return;
        }

        try {
            const selected = await openDialog({
                multiple: item.multiple,
                filters: [{ name: t(item.labelKey), extensions: item.accept.replace(/\./g, '').split(',') }],
            });

            const paths: string[] = selected === null ? [] : Array.isArray(selected) ? selected : [selected];

            for (const filePath of paths) {
                const name = filePath.split(/[\\/]/).pop() || filePath;
                const iconMap: Record<FileMenuKey, React.ReactNode> = {
                    document: <FileText className="w-3.5 h-3.5" />,
                    image_ocr: <FileImage className="w-3.5 h-3.5" />,
                    url: <Link className="w-3.5 h-3.5" />,
                };
                const previewUrl = item.key === 'image_ocr' ? convertFileSrc(filePath) : undefined;
                const fileId = `${item.key}-${Date.now()}-${Math.random()}`;
                setAttachedFiles(prev => [...prev, {
                    id: fileId,
                    name,
                    filePath,
                    fileType: item.key,
                    icon: iconMap[item.key],
                    previewUrl,
                    isParsing: true,
                }]);
                parseTempFile(fileId, filePath, null);
            }
        } catch (err) {
            console.error('[InputArea] openDialog error:', err);
        }
    };

    const handleAddUrl = () => {
        const url = urlInputValue.trim();
        if (!url) { setUrlInputOpen(false); return; }
        const name = (() => {
            try { return new URL(url).hostname; } catch { return url.slice(0, 40); }
        })();
        const fileId = `url-${Date.now()}`;
        setAttachedFiles(prev => [...prev, {
            id: fileId,
            name,
            filePath: url,
            fileType: 'url',
            icon: <Link className="w-3.5 h-3.5" />,
            isParsing: true,
        }]);
        setUrlInputValue('');
        setUrlInputOpen(false);
        parseTempFile(fileId, null, url);
    };

    const hasContent = input.trim() || attachedFiles.length > 0 || !!quotedMessage || mentionedSources.length > 0 || mentionedTags.length > 0;

    return (
        <div className="relative border-t border-stroke-divider bg-surface-base px-4 py-3">

            {ragEnabled && recommendedTags.length > 0 && (
                <div className="flex items-center gap-1.5 mb-2 flex-wrap">
                    <span className="text-[11px] text-text-tertiary mr-0.5">{t('chat.recommended_tags')}</span>
                    {recommendedTags.map(tag => (
                        <button
                            key={tag.id}
                            onClick={() => setMentionedTags(prev =>
                                prev.some(t => t.name === tag.name) ? prev : [...prev, { id: tag.id, name: tag.name }]
                            )}
                            className="flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] bg-surface-subtle hover:bg-accent-light2 text-text-secondary hover:text-accent-default border border-stroke-divider/50 transition-colors"
                        >
                            <Hash className="w-2.5 h-2.5" />
                            {tag.name}
                        </button>
                    ))}
                </div>
            )}

            {isTagOpen && (
                <div className="absolute bottom-full left-4 right-4 mb-2 bg-surface-flyout border border-stroke-divider rounded-xl shadow-2xl overflow-hidden z-50 animate-in slide-in-from-bottom-2 duration-150">
                    <div className="px-3 py-2 border-b border-stroke-divider flex items-center gap-2">
                        <Hash className="w-3.5 h-3.5 text-accent-default" />
                        <span className="text-fs-sm text-text-tertiary font-medium">
                            {tagQuery ? t('chat.tag_search_title', { query: tagQuery }) : t('chat.tag_recent_title')}
                        </span>
                        <span className="ml-auto text-[10px] text-text-tertiary">{t('chat.tag_scope_hint')}</span>
                    </div>
                    <div className="max-h-[200px] overflow-y-auto">
                        {tagResults.length === 0 ? (
                            <div className="px-4 py-5 text-center text-text-tertiary text-fs-base">
                                {tagQuery ? t('chat.tag_not_found') : t('chat.tag_empty')}
                            </div>
                        ) : (
                            tagResults.map((tag, i) => (
                                <button
                                    key={tag.id}
                                    onClick={() => selectTag(tag)}
                                    onMouseEnter={() => setTagIndex(i)}
                                    className={`w-full text-left px-3 py-2 flex items-center gap-2.5 transition-colors border-l-2 ${i === tagIndex
                                            ? 'bg-accent-light2 border-accent-default'
                                            : 'hover:bg-surface-subtle border-transparent'
                                        }`}
                                >
                                    <div className="w-6 h-6 shrink-0 rounded-md flex items-center justify-center bg-accent-light2 text-accent-default">
                                        <Hash className="w-3 h-3" />
                                    </div>
                                    <span className="text-fs-base font-medium text-text-primary">{tag.name}</span>
                                    {tag.useCount !== undefined && (
                                        <span className="ml-auto text-fs-sm text-text-tertiary">{t('chat.tag_use_count', { count: tag.useCount })}</span>
                                    )}
                                </button>
                            ))
                        )}
                    </div>
                </div>
            )}

            {isMentionOpen && (
                <div className="absolute bottom-full left-4 right-4 mb-2 bg-surface-flyout border border-stroke-divider rounded-xl shadow-2xl overflow-hidden z-50 animate-in slide-in-from-bottom-2 duration-150">
                    <div className="px-3 py-2 border-b border-stroke-divider flex items-center gap-2">
                        <AtSign className="w-3.5 h-3.5 text-accent-default" />
                        <span className="text-fs-sm text-text-tertiary font-medium">
                            {mentionQuery ? t('chat.mention_search_title', { query: mentionQuery }) : t('chat.mention_recent_title')}
                        </span>
                        <span className="ml-auto text-[10px] text-text-tertiary">{t('chat.mention_scope_hint')}</span>
                    </div>
                    <div className="max-h-[220px] overflow-y-auto">
                        {mentionResults.length === 0 ? (
                            <div className="px-4 py-5 text-center text-text-tertiary text-fs-base">
                                {mentionQuery ? t('chat.mention_not_found') : t('chat.mention_empty')}
                            </div>
                        ) : (
                            mentionResults.map((src, i) => (
                                <button
                                    key={src.id}
                                    onClick={() => selectMention(src)}
                                    onMouseEnter={() => setMentionIndex(i)}
                                    className={`w-full text-left px-3 py-2.5 flex items-start gap-2.5 transition-colors border-l-2 ${i === mentionIndex
                                            ? 'bg-accent-light2 border-accent-default'
                                            : 'hover:bg-surface-subtle border-transparent'
                                        }`}
                                >
                                    <div className="w-7 h-7 shrink-0 rounded-lg flex items-center justify-center mt-0.5 bg-accent-light2 text-accent-default">
                                        {src.source_type === 'url' ? <Link className="w-3.5 h-3.5" /> : <FileText className="w-3.5 h-3.5" />}
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="text-fs-base font-medium text-text-primary truncate">{src.title}</div>
                                    </div>
                                </button>
                            ))
                        )}
                    </div>
                </div>
            )}

            {urlInputOpen && (
                <div className="absolute bottom-full left-4 right-4 mb-2 bg-surface-flyout border border-stroke-divider rounded-xl shadow-2xl overflow-hidden z-50 animate-in slide-in-from-bottom-2 duration-150">
                    <div className="px-3 py-2 border-b border-stroke-divider flex items-center gap-2">
                        <Link className="w-3.5 h-3.5 text-accent-default" />
                        <span className="text-fs-sm text-text-tertiary font-medium">{t('chat.add_url_hint')}</span>
                    </div>
                    <div className="flex items-center gap-2 p-3">
                        <input
                            ref={urlInputRef}
                            value={urlInputValue}
                            onChange={e => setUrlInputValue(e.target.value)}
                            onKeyDown={e => {
                                if (e.key === 'Enter') { e.preventDefault(); handleAddUrl(); }
                                if (e.key === 'Escape') { setUrlInputOpen(false); setUrlInputValue(''); }
                            }}
                            placeholder={t('chat.url_placeholder')}
                            className="flex-1 text-fs-base bg-surface-subtle border border-stroke-divider rounded-lg px-3 py-1.5 outline-none focus:ring-1 focus:ring-accent-default text-text-primary placeholder:text-text-tertiary"
                        />
                        <button
                            onClick={handleAddUrl}
                            className="px-3 py-1.5 text-fs-sm bg-accent-default text-white rounded-lg hover:bg-accent-default/90 transition-colors"
                        >
                            {t('chat.add_url_button')}
                        </button>
                        <button
                            onClick={() => { setUrlInputOpen(false); setUrlInputValue(''); }}
                            className="p-1.5 text-text-tertiary hover:text-text-secondary rounded-md transition-colors"
                        >
                            <X className="w-3.5 h-3.5" />
                        </button>
                    </div>
                </div>
            )}

            <div className="flex flex-col w-full bg-surface-card rounded-xl border border-stroke-divider/50 shadow-sm">

                {(attachedFiles.length > 0 || quotedMessage || mentionedSources.length > 0 || mentionedTags.length > 0) && (
                    <div className="flex flex-wrap gap-2 px-3 pt-3 pb-2 border-b border-stroke-divider/50">

                        {mentionedTags.map(tag => (
                            <div key={tag.id} className="group relative flex items-center gap-1 bg-emerald-50 dark:bg-emerald-950/30 rounded-xl px-2.5 py-1 border border-emerald-300/50 dark:border-emerald-700/40">
                                <Hash className="w-3 h-3 text-emerald-600 dark:text-emerald-400 shrink-0" />
                                <span className="text-fs-sm text-emerald-700 dark:text-emerald-300 truncate max-w-[120px] font-medium">{tag.name}</span>
                                <button
                                    onClick={() => setMentionedTags(prev => prev.filter(t => t.id !== tag.id))}
                                    className="ml-0.5 text-text-tertiary opacity-0 group-hover:opacity-100 hover:text-red-500 transition-all"
                                >
                                    <X className="w-3 h-3" strokeWidth={3} />
                                </button>
                            </div>
                        ))}

                        {mentionedSources.map(src => (
                            <div key={src.id} className="group relative flex items-center gap-1.5 bg-accent-light2 rounded-xl px-2.5 py-1 border border-accent-default/30">
                                <AtSign className="w-3 h-3 text-accent-default shrink-0" />
                                <span className="text-fs-sm text-accent-default truncate max-w-[120px] font-medium">{src.title}</span>
                                <button
                                    onClick={() => setMentionedSources(prev => prev.filter(s => s.id !== src.id))}
                                    className="ml-0.5 text-text-tertiary opacity-0 group-hover:opacity-100 hover:text-red-500 transition-all"
                                >
                                    <X className="w-3 h-3" strokeWidth={3} />
                                </button>
                            </div>
                        ))}

                        {quotedMessage && (
                            <div className="group relative flex items-start gap-2 bg-surface-subtle rounded-xl w-full max-w-[380px] p-2.5 pr-8 border border-stroke-divider">
                                <div className="w-7 h-7 shrink-0 rounded-lg bg-accent-light2 text-accent-default flex items-center justify-center">
                                    <Quote className="w-3.5 h-3.5" />
                                </div>
                                <div className="flex flex-col flex-1 overflow-hidden">
                                    <span className="text-fs-sm text-text-tertiary font-medium mb-0.5">{t('chat.quote_message')}</span>
                                    <span className="text-fs-base text-text-primary line-clamp-2 leading-snug">{quotedMessage.content}</span>
                                </div>
                                <button
                                    onClick={() => setQuotedMessage(null)}
                                    className="absolute top-1.5 right-1.5 p-0.5 rounded-full bg-surface-subtle border border-stroke-divider text-text-tertiary opacity-0 group-hover:opacity-100 hover:text-red-500 transition-all"
                                >
                                    <X className="w-3 h-3" strokeWidth={3} />
                                </button>
                            </div>
                        )}

                        {attachedFiles.map(file => (
                            file.fileType === 'image_ocr' && file.previewUrl ? (
                                <div key={file.id} className="group relative shrink-0 rounded-xl overflow-hidden border border-stroke-divider w-16 h-16">
                                    <img
                                        src={file.previewUrl}
                                        alt={file.name}
                                        className="w-full h-full object-cover"
                                    />
                                    {file.isParsing ? (
                                        <div className="absolute inset-0 bg-black/60 flex items-center justify-center">
                                            <Loader2 className="w-4 h-4 text-white animate-spin" />
                                        </div>
                                    ) : file.isError ? (
                                        <div className="absolute inset-0 bg-red-500/70 flex items-center justify-center">
                                            <span className="text-[9px] text-white font-medium">{t('chat.image_failed')}</span>
                                        </div>
                                    ) : (
                                        <div className="absolute inset-x-0 bottom-0 bg-black/50 px-1 py-0.5">
                                            <span className="text-[9px] text-white font-medium block truncate leading-tight">{t('chat.ocr')}</span>
                                        </div>
                                    )}
                                    <button
                                        onClick={() => setAttachedFiles(prev => prev.filter(f => f.id !== file.id))}
                                        className="absolute top-0.5 right-0.5 p-0.5 rounded-full bg-black/50 text-white opacity-0 group-hover:opacity-100 hover:bg-red-500 transition-all"
                                    >
                                        <X className="w-2.5 h-2.5" strokeWidth={3} />
                                    </button>
                                </div>
                            ) : (
                                <div key={file.id} className={`group relative flex items-center gap-2 bg-surface-card rounded-xl h-10 px-2.5 pr-7 border max-w-[200px] ${file.isError ? 'border-red-400' : 'border-stroke-divider'}`}>
                                    <div className={`shrink-0 ${file.isError ? 'text-red-400' : 'text-accent-default'}`}>
                                        {file.isParsing ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : file.icon}
                                    </div>
                                    <div className="flex flex-col overflow-hidden">
                                        <span className="text-fs-sm font-medium text-text-primary truncate leading-tight">{file.name}</span>
                                        <span className={`text-[10px] uppercase leading-tight ${file.isError ? 'text-red-400' : 'text-text-tertiary'}`}>
                                            {file.isParsing ? t('chat.parsing') : file.isError ? t('chat.parse_failed') : file.fileType === 'url' ? t('chat.file_type_url') : t('chat.file_type_doc')}
                                        </span>
                                    </div>
                                    <button
                                        onClick={() => setAttachedFiles(prev => prev.filter(f => f.id !== file.id))}
                                        className="absolute top-0.5 right-0.5 p-0.5 rounded-full bg-surface-subtle text-text-tertiary opacity-0 group-hover:opacity-100 hover:text-red-500 transition-all"
                                    >
                                        <X className="w-2.5 h-2.5" strokeWidth={3} />
                                    </button>
                                </div>
                            )
                        ))}
                    </div>
                )}

                <div className="flex items-end gap-2 px-2 pt-2">
                    <textarea
                        ref={textareaRef}
                        value={input}
                        onChange={handleInput}
                        onKeyDown={handleKeyDown}
                        placeholder={t('chat.placeholder')}
                        className="flex-1 max-h-[200px] min-h-[40px] bg-transparent border-0 outline-none focus:outline-none focus:ring-0 resize-none py-2 px-1 text-fs-base text-text-primary placeholder:text-text-tertiary"
                        rows={1}
                    />
                    <button
                        onClick={handleSend}
                        disabled={!hasContent || isGenerating || attachedFiles.some(f => f.isParsing)}
                        className={`mb-1.5 p-2 rounded-lg shrink-0 transition-all ${hasContent && !isGenerating && !attachedFiles.some(f => f.isParsing)
                                ? 'bg-accent-default text-white hover:bg-accent-default/90 shadow-sm'
                                : 'text-text-tertiary cursor-not-allowed'
                            }`}
                    >
                        {isGenerating || attachedFiles.some(f => f.isParsing)
                            ? <Loader2 className="w-4 h-4 animate-spin" />
                            : <Send className="w-4 h-4" />}
                    </button>
                </div>

                <div className="flex items-center gap-1.5 px-3 pb-2 pt-0.5">

                    <div ref={fileMenuRef} className="relative">
                        <button
                            onClick={() => setFileMenuOpen(prev => !prev)}
                            className={`flex items-center justify-center w-6 h-6 rounded-full text-fs-sm transition-all ${fileMenuOpen
                                    ? 'bg-accent-default text-white'
                                    : 'text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle'
                                }`}
                            title={t('chat.add_file')}
                        >
                            <Plus className="w-3.5 h-3.5" />
                        </button>

                        {fileMenuOpen && (
                            <div className="absolute bottom-full left-0 mb-1.5 bg-surface-flyout border border-stroke-divider rounded-lg shadow-lg z-50 w-40 overflow-hidden animate-in slide-in-from-bottom-2 duration-150">
                                {FILE_MENU_ITEMS.map(item => (
                                    <button
                                        key={item.key}
                                        onClick={() => handlePickFile(item)}
                                        className="w-full flex items-center gap-2 px-2.5 py-1.5 hover:bg-surface-subtle transition-colors text-left group"
                                    >
                                        <div className="w-5 h-5 shrink-0 rounded-md bg-accent-light2 text-accent-default flex items-center justify-center group-hover:bg-accent-default group-hover:text-white transition-colors">
                                            {item.icon}
                                        </div>
                                        <span className="text-fs-sm text-text-primary">{t(item.labelKey)}</span>
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    <button
                        onClick={() => setRagEnabled(prev => !prev)}
                        className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-fs-sm transition-all ${ragEnabled
                                ? 'bg-accent-light2 text-accent-default border border-accent-default/30'
                                : 'text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle'
                            }`}
                        title={t('chat.knowledge_tooltip')}
                    >
                        <Database className="w-3 h-3" />
                        <span>{t('chat.knowledge_base')}</span>
                    </button>

                    <button
                        onClick={() => setWebEnabled(prev => !prev)}
                        className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-fs-sm transition-all ${webEnabled
                                ? 'bg-accent-light2 text-accent-default border border-accent-default/30'
                                : 'text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle'
                            }`}
                        title={t('chat.web_tooltip')}
                    >
                        <Globe className="w-3 h-3" />
                        <span>{t('chat.web_search')}</span>
                    </button>

                    {supportsThinking && (
                        <button
                            onClick={() => setThinkingMode(thinkingMode === 'think' ? 'normal' : 'think')}
                            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-fs-sm transition-all ${thinkingMode === 'think'
                                    ? 'bg-accent-light2 text-accent-default border border-accent-default/30'
                                    : 'text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle'
                                }`}
                            title={thinkingMode === 'think' ? t('chat.think_mode_active') : t('chat.think_mode_inactive')}
                        >
                            <Brain className="w-3 h-3" />
                            <span>{thinkingMode === 'think' ? t('chat.think_mode') : t('chat.normal_mode')}</span>
                        </button>
                    )}

                    <div className="flex-1" />

                    <span className="text-[11px] text-text-tertiary select-none">
                        {t('chat.hint_at_tag')}
                    </span>
                </div>
            </div>
        </div>
    );
};
