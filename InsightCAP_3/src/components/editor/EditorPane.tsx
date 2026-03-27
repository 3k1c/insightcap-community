import { useEffect, useRef, useCallback, useState } from 'react';
import { asBlob } from 'html-docx-js-typescript';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';
import { FontFamily } from '@tiptap/extension-font-family';
import { TextStyle } from '@tiptap/extension-text-style';
import { Color } from '@tiptap/extension-color';
import Highlight from '@tiptap/extension-highlight';
import CharacterCount from '@tiptap/extension-character-count';
import { Table } from '@tiptap/extension-table';
import TableRow from '@tiptap/extension-table-row';
import TableHeader from '@tiptap/extension-table-header';
import TableCell from '@tiptap/extension-table-cell';
import ResizableImage from 'tiptap-extension-resize-image';
import { Extension } from '@tiptap/core';
import { Grid3x3, ImagePlus, FileText, FileCode, Printer, Baseline, Highlighter, Search, Clock, X as XIcon, ChevronDown, ChevronRight, Underline as UnderlineIcon, AlignLeft, AlignCenter, AlignRight, AlignJustify, Minus, Link2 } from 'lucide-react';
import { save } from '@tauri-apps/plugin-dialog';
import { tauriCmd } from '../../lib/tauri';
import { useUiStore } from '../../stores/uiStore';
import { useT } from '../../hooks/useT';
import { SearchReplaceExtension } from './SearchReplaceExtension';
import { SearchReplaceBar } from './SearchReplaceBar';
import { EditorContextMenu, AiAction } from './EditorContextMenu';
import UnderlineExt from '@tiptap/extension-underline';
import TextAlign from '@tiptap/extension-text-align';
import TiptapLink from '@tiptap/extension-link';

// Custom LineHeight extension (paragraph/heading attribute)
const LineHeight = Extension.create({
    name: 'lineHeight',
    addOptions() { return { types: ['paragraph', 'heading'] }; },
    addGlobalAttributes() {
        return [{
            types: this.options.types,
            attributes: {
                lineHeight: {
                    default: null,
                    parseHTML: el => el.style.lineHeight || null,
                    renderHTML: attrs => {
                        if (!attrs.lineHeight) return {};
                        return { style: `line-height: ${attrs.lineHeight}` };
                    },
                },
            },
        }];
    },
    addCommands() {
        return {
            setLineHeight: (lineHeight: string) => ({ commands }: any) =>
                this.options.types.every((type: string) =>
                    commands.updateAttributes(type, { lineHeight })
                ),
            unsetLineHeight: () => ({ commands }: any) =>
                this.options.types.every((type: string) =>
                    commands.resetAttributes(type, 'lineHeight')
                ),
        } as any;
    },
});

// Custom FontSize extension (TextStyle attribute)
const FontSize = Extension.create({
    name: 'fontSize',
    addOptions() { return { types: ['textStyle'] }; },
    addGlobalAttributes() {
        return [{
            types: this.options.types,
            attributes: {
                fontSize: {
                    default: null,
                    parseHTML: el => el.style.fontSize || null,
                    renderHTML: attrs => {
                        if (!attrs.fontSize) return {};
                        return { style: `font-size: ${attrs.fontSize}` };
                    },
                },
            },
        }];
    },
    addCommands() {
        return {
            setFontSize: (size: string) => ({ chain }: any) =>
                chain().setMark('textStyle', { fontSize: size }).run(),
            unsetFontSize: () => ({ chain }: any) =>
                chain().setMark('textStyle', { fontSize: null }).removeEmptyTextStyle().run(),
        } as any;
    },
});

// ── Phase C: 版本歷史快照 ──────────────────────────────────────────────────
interface HistorySnapshot {
    timestamp: string;
    content: string;
    wordCount: number;
}
const HISTORY_MAX = 10;

function getHistoryKey(filePath: string) { return `editor_history:${filePath}`; }

function saveSnapshot(filePath: string, content: string, wordCount: number) {
    const key = getHistoryKey(filePath);
    const existing: HistorySnapshot[] = JSON.parse(localStorage.getItem(key) ?? '[]');
    if (existing.length > 0 && existing[0].content === content) return;
    const updated = [{ timestamp: new Date().toISOString(), content, wordCount }, ...existing].slice(0, HISTORY_MAX);
    localStorage.setItem(key, JSON.stringify(updated));
}

function getSnapshots(filePath: string): HistorySnapshot[] {
    return JSON.parse(localStorage.getItem(getHistoryKey(filePath)) ?? '[]');
}
// ─────────────────────────────────────────────────────────────────────────────

const AUTOSAVE_DELAY_MS = 1000;

const FONT_SIZES = ['8', '9', '10', '11', '12', '14', '16', '18', '20', '24', '28', '32', '36', '48', '72'];
const DEFAULT_FONT_SIZE = '12';

const TEXT_COLOR_VALUES = [
    { key: 'editor.colorDefault', value: '' },
    { key: 'editor.colorBlack', value: '#000000' },
    { key: 'editor.colorGray', value: '#6b7280' },
    { key: 'editor.colorRed', value: '#ef4444' },
    { key: 'editor.colorOrange', value: '#f97316' },
    { key: 'editor.colorYellow', value: '#eab308' },
    { key: 'editor.colorGreen', value: '#22c55e' },
    { key: 'editor.colorBlue', value: '#3b82f6' },
    { key: 'editor.colorPurple', value: '#a855f7' },
];

const HIGHLIGHT_COLOR_VALUES = [
    { key: 'editor.hlClear', value: '' },
    { key: 'editor.hlYellow', value: '#fef08a' },
    { key: 'editor.hlGreen', value: '#bbf7d0' },
    { key: 'editor.hlBlue', value: '#bfdbfe' },
    { key: 'editor.hlPink', value: '#fbcfe8' },
    { key: 'editor.hlOrange', value: '#fed7aa' },
];

const LINE_SPACINGS = [
    { label: 'Single', value: '1' },
    { label: '1.15×', value: '1.15' },
    { label: '1.5×', value: '1.5' },
    { label: 'Double', value: '2' },
    { label: '2.5×', value: '2.5' },
    { label: '3×', value: '3' },
];
const DEFAULT_LINE_SPACING = '1.5';


const FONTS_STATIC = [
    { label: 'Microsoft JhengHei', value: 'Microsoft JhengHei, sans-serif' },
    { label: 'PMingLiU', value: 'PMingLiU, serif' },
    { label: 'DFKai-SB', value: 'DFKai-SB, serif' },
    { label: 'Arial', value: 'Arial, sans-serif' },
    { label: 'Georgia', value: 'Georgia, serif' },
    { label: 'Courier New', value: 'Courier New, monospace' },
];

export interface EditorPaneProps {
    /** kb_path 下的相對路徑，例如 "notes/my-doc.md" */
    filePath: string;
    onSaveStatus?: (status: 'saving' | 'saved' | 'error') => void;
}

export function EditorPane({ filePath, onSaveStatus }: EditorPaneProps) {
    const T = useT();
    const TEXT_COLORS = TEXT_COLOR_VALUES.map(c => ({ label: T(c.key as any), value: c.value }));
    const HIGHLIGHT_COLORS = HIGHLIGHT_COLOR_VALUES.map(c => ({ label: T(c.key as any), value: c.value }));
    const FONTS = [{ label: T('editor.defaultFont'), value: '' }, ...FONTS_STATIC];
    const HEADING_ITEMS = [
        { label: T('editor.bodyText'), level: 0 },
        { label: `${T('editor.heading')} 1`, level: 1 },
        { label: `${T('editor.heading')} 2`, level: 2 },
        { label: `${T('editor.heading')} 3`, level: 3 },
        { label: `${T('editor.heading')} 4`, level: 4 },
        { label: `${T('editor.heading')} 5`, level: 5 },
        { label: `${T('editor.heading')} 6`, level: 6 },
    ];
    const autosaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
    const isLoadingRef = useRef(false);
    const dirtyContentRef = useRef<string | null>(null); // 未儲存的內容，卸載時用於緊急儲存
    const localSavePathRef = useRef<string | null>(null); // Ctrl+S 記憶的本地儲存路徑
    const imgAssetMap = useRef<Map<string, string>>(new Map()); // base64 dataUrl → ./assets/filename
    const [showExport, setShowExport] = useState(false);
    const [showColorPicker, setShowColorPicker] = useState(false);
    const [showHighlightPicker, setShowHighlightPicker] = useState(false);
    const [wordCount, setWordCount] = useState(0);
    const [searchMode, setSearchMode] = useState<'find' | 'replace' | null>(null);
    const [searchTerm, setSearchTerm] = useState('');
    const [replaceTerm, setReplaceTerm] = useState('');
    const [editorContextMenu, setEditorContextMenu] = useState<{ x: number; y: number; inTable: boolean; selectedText: string; selectionFrom: number; selectionTo: number } | null>(null);
    const [aiPanel, setAiPanel] = useState<{ action: string; text: string; loading: boolean; selectionFrom: number; selectionTo: number; x: number; y: number } | null>(null);
    const aiUnlistenRef = useRef<(() => void) | null>(null);
    const aiTaskRef = useRef<{ action: AiAction; selectedText: string; from: number; to: number } | null>(null);
    const [showHistory, setShowHistory] = useState(false);
    const [historyTick, setHistoryTick] = useState(0); // force re-render history list
    const [savePathHint, setSavePathHint] = useState<string | null>(null);
    const { setPendingEditorSelection, openEditor, closeEditorTab } = useUiStore();
    const [showFileMenu, setShowFileMenu] = useState(false);
    const [showHeadingMenu, setShowHeadingMenu] = useState(false);
    const [showListMenu, setShowListMenu] = useState(false);
    const [showTableMenu, setShowTableMenu] = useState(false);
    const [tableGridHover, setTableGridHover] = useState<{ row: number; col: number } | null>(null);
    const [isDragOver, setIsDragOver] = useState(false);

    const editor = useEditor({
        extensions: [
            StarterKit,
            Markdown.configure({
                html: false,
                transformPastedText: true,
                transformCopiedText: false,
            }),
            TextStyle,
            FontFamily,
            FontSize,
            LineHeight,
            Color,
            Highlight.configure({ multicolor: true }),
            CharacterCount,
            SearchReplaceExtension,
            Table.configure({ resizable: true }),
            TableRow,
            TableHeader,
            TableCell,
            ResizableImage,
            UnderlineExt,
            TextAlign.configure({ types: ['heading', 'paragraph'] }),
            TiptapLink.configure({ openOnClick: false }),
        ],
        content: '',
        editorProps: {
            attributes: {
                class: 'prose prose-sm focus:outline-none max-w-none w-full min-h-[500px] leading-relaxed',
            },
        },
        onCreate: ({ editor }) => {
            setWordCount(editor.state.doc.textContent.length);
        },
        onUpdate: ({ editor }) => {
            setWordCount(editor.state.doc.textContent.length);
            if (isLoadingRef.current) return;

            // 立即記錄 dirty 內容，確保卸載時可緊急儲存（base64 → 相對路徑）
            let _md = (editor.storage as any).markdown.getMarkdown();
            for (const [dataUrl, relPath] of imgAssetMap.current) {
                _md = _md.split(dataUrl).join(relPath);
            }
            dirtyContentRef.current = _md;

            if (autosaveTimer.current) clearTimeout(autosaveTimer.current);

            autosaveTimer.current = setTimeout(async () => {
                try {
                    onSaveStatus?.('saving');
                    const markdown = dirtyContentRef.current!;
                    await tauriCmd.saveDocument(filePath, markdown);
                    saveSnapshot(filePath, markdown, editor.state.doc.textContent.length);
                    setHistoryTick(t => t + 1);
                    onSaveStatus?.('saved');
                    dirtyContentRef.current = null; // 儲存成功後清除
                } catch (e) {
                    console.error('[EditorPane] auto-save failed:', e);
                    onSaveStatus?.('error');
                }
            }, AUTOSAVE_DELAY_MS);
        },
    });

    const loadFile = useCallback(async () => {
        if (!editor || !filePath) return;

        isLoadingRef.current = true;
        imgAssetMap.current.clear();
        try {
            let content = await tauriCmd.openDocument(filePath) ?? '';

            // 將 ./assets/ 相對路徑還原為 base64 以供編輯器顯示，同時重建 imgAssetMap
            const assetRe = /!\[([^\]]*)\]\((\.\/assets\/[^)"\s]+)\)/g;
            const matches = [...content.matchAll(assetRe)];
            if (matches.length > 0) {
                const settings = await tauriCmd.getSettings();
                const kbPath = settings.knowledge.kbPath.replace(/[/\\]+$/, '').replace(/\\/g, '/');
                const docDir = filePath.split('/').slice(0, -1).join('/');
                for (const match of matches) {
                    const [full, alt, relPath] = match;
                    const absPath = [kbPath, docDir, relPath.replace('./', '')].filter(Boolean).join('/');
                    try {
                        const dataUrl = await tauriCmd.readImageBase64(absPath);
                        imgAssetMap.current.set(dataUrl, relPath);
                        content = content.replace(full, `![${alt}](${dataUrl})`);
                    } catch { /* 圖片遺失時保留原路徑 */ }
                }
            }

            editor.commands.setContent(content || '');

            // 新建（空白）文件時套用設定中的預設字型／字號／行距
            if (!content.trim()) {
                const es = await tauriCmd.getSettings().then(s => s.editor).catch(() => null);
                if (es) {
                    editor.chain().focus()
                        .selectAll()
                        .run();
                    if (es.defaultFont) {
                        (editor.chain() as any).setFontFamily(es.defaultFont).run();
                    }
                    if (es.defaultFontSize) {
                        (editor.chain().focus() as any).setFontSize(`${es.defaultFontSize}pt`).run();
                    }
                    if (es.defaultLineSpacing) {
                        (editor.chain().focus() as any).setLineHeight(es.defaultLineSpacing).run();
                    }
                    editor.commands.setTextSelection(0);
                }
            }
        } catch {
            await tauriCmd.createDocument(filePath, 'md').catch(() => { });
            editor.commands.setContent('');
        } finally {
            isLoadingRef.current = false;
        }
    }, [editor, filePath]);

    useEffect(() => {
        loadFile();
        return () => {
            if (autosaveTimer.current) {
                clearTimeout(autosaveTimer.current);
                // 卸載時若有未儲存內容，立即觸發儲存（fire-and-forget）
                if (dirtyContentRef.current !== null) {
                    tauriCmd.saveDocument(filePath, dirtyContentRef.current).catch(e =>
                        console.error('[EditorPane] unmount save failed:', e)
                    );
                    dirtyContentRef.current = null;
                }
            }
        };
    }, [loadFile]);

    // 監聽「加入編輯器」事件
    useEffect(() => {
        const handler = (e: Event) => {
            const { content } = (e as CustomEvent<{ content: string }>).detail;
            if (!editor || !content) return;
            editor.chain().focus().insertContentAt(editor.state.doc.content.size, '\n\n' + content).run();
        };
        window.addEventListener('insert-to-editor', handler);
        return () => window.removeEventListener('insert-to-editor', handler);
    }, [editor]);

    // 監聽從對話拖入編輯器事件
    useEffect(() => {
        const onEnter = () => setIsDragOver(true);
        const onLeave = () => setIsDragOver(false);
        const onDrop = (e: Event) => {
            const { content, x, y } = (e as CustomEvent<{ content: string; x: number; y: number }>).detail;
            if (!editor || !content) return;
            setIsDragOver(false);
            const coords = editor.view.posAtCoords({ left: x, top: y });
            const insertPos = coords ? coords.pos : editor.state.doc.content.size;
            editor.chain().focus().insertContentAt(insertPos, content).run();
        };
        window.addEventListener('editor-drag-enter', onEnter);
        window.addEventListener('editor-drag-leave', onLeave);
        window.addEventListener('drop-to-editor', onDrop);
        return () => {
            window.removeEventListener('editor-drag-enter', onEnter);
            window.removeEventListener('editor-drag-leave', onLeave);
            window.removeEventListener('drop-to-editor', onDrop);
        };
    }, [editor]);

    // 監聽「覆蓋到編輯器」事件（Phase C: 超過原文 3 倍時確認）
    useEffect(() => {
        const handler = (e: Event) => {
            const { from, to, content } = (e as CustomEvent<{ from: number; to: number; content: string }>).detail;
            if (!editor) return;
            const originalText = editor.state.doc.textBetween(from, to, '\n');
            if (content.length > originalText.length * 3 && originalText.length > 0) {
                const confirmed = window.confirm(
                    `AI 修改的內容比原文長很多（${content.length} 字 vs ${originalText.length} 字），確認覆蓋？`
                );
                if (!confirmed) return;
            }
            editor.chain().focus().setTextSelection({ from, to }).insertContent(content).run();
            setPendingEditorSelection(null);
        };
        window.addEventListener('overwrite-editor-selection', handler);
        return () => window.removeEventListener('overwrite-editor-selection', handler);
    }, [editor, setPendingEditorSelection]);

    // 點擊外部關閉下拉選單
    useEffect(() => {
        if (!showExport && !showColorPicker && !showHighlightPicker && !editorContextMenu && !showFileMenu && !showHeadingMenu && !showListMenu && !showTableMenu) return;
        const handler = () => {
            setShowExport(false);
            setShowColorPicker(false);
            setShowHighlightPicker(false);
            setEditorContextMenu(null);
            setShowFileMenu(false);
            setShowHeadingMenu(false);
            setShowListMenu(false);
            setShowTableMenu(false);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, [showExport, showColorPicker, showHighlightPicker, editorContextMenu, showFileMenu, showHeadingMenu, showListMenu, showTableMenu]);

    // Ctrl+F / Ctrl+H 快捷鍵
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if (!e.ctrlKey) return;
            if (e.key === 'f' || e.key === 'F') {
                e.preventDefault();
                setSearchMode(prev => prev ? null : 'find');
            } else if (e.key === 'h' || e.key === 'H') {
                e.preventDefault();
                setSearchMode(prev => (prev === 'replace' ? null : 'replace'));
            }
        };
        window.addEventListener('keydown', handler);
        return () => window.removeEventListener('keydown', handler);
    }, []);

    // 關閉搜尋時清除 highlights
    useEffect(() => {
        if (!searchMode && editor) {
            setSearchTerm('');
            (editor.commands as any).setSearchTerm('');
        }
    }, [searchMode, editor]);

    // 編輯器右鍵選單（含 AI 功能 + 標準編輯 + 表格操作）
    const handleEditorContextMenu = (e: React.MouseEvent) => {
        if (!editor) return;
        e.preventDefault();
        const inTable = editor.isActive('table') || editor.isActive('tableCell') || editor.isActive('tableHeader');
        const { from, to } = editor.state.selection;
        const selectedText = from === to ? '' : editor.state.doc.textBetween(from, to, ' ');
        setEditorContextMenu({ x: e.clientX, y: e.clientY, inTable, selectedText, selectionFrom: from, selectionTo: to });
    };

    // AI 動作處理
    const handleAiAction = async (action: AiAction, selectedText: string, from: number, to: number) => {
        // 停止前一次串流
        aiUnlistenRef.current?.();
        aiTaskRef.current = { action, selectedText, from, to };

        const styleGuideMap: Record<string, string> = {
            formal: '正式、專業的書面語',
            casual: '親切、自然的口語',
            academic: '學術論文風格，嚴謹且引經據典',
            journalistic: '新聞報導風格，客觀簡潔',
            narrative: '故事敘述風格，生動流暢',
            bullet: '條列式要點，清晰易讀',
        };

        let systemPrompt = '';
        let actionLabel = '';

        if (action.type === 'rewrite') {
            const styleGuide = styleGuideMap[action.style] || action.style;
            actionLabel = `AI 改寫・${action.styleLabel}`;
            systemPrompt = `你是一位專業文字編輯。請將使用者提供的文字，以「${styleGuide}」的風格重新改寫。只輸出改寫後的文字，不要有任何解釋或說明。`;
        } else if (action.type === 'shorten') {
            actionLabel = 'AI 縮寫';
            systemPrompt = '你是一位專業文字編輯。請將使用者提供的文字精簡縮短，保留核心意思，去除冗餘。只輸出縮寫後的文字，不要有任何解釋或說明。';
        } else {
            // action.type === 'expand'
            const lengthGuide = action.length === 'short'
                ? '適度擴展，約增加 50% 篇幅'
                : action.length === 'medium'
                    ? '中等擴展，約增加至原文一倍篇幅'
                    : '大幅擴展，增加豐富的細節、例子與說明，篇幅約為原文兩倍以上';
            actionLabel = `AI 擴寫・${action.lengthLabel}`;
            systemPrompt = `你是一位專業文字編輯。請將使用者提供的文字擴展延伸（${lengthGuide}），增加細節、例子或說明。只輸出擴寫後的文字，不要有任何解釋或說明。`;
        }

        // 計算選取文字底部座標，讓面板浮在選取文字下方
        let panelX = 120;
        let panelY = 120;
        if (editor) {
            try {
                const endCoords = editor.view.coordsAtPos(Math.min(to, editor.state.doc.content.size));
                const startCoords = editor.view.coordsAtPos(from);
                panelX = Math.min(startCoords.left, window.innerWidth - 520);
                panelY = endCoords.bottom + 10;
                // 若面板超出底部，改顯示在選取文字上方
                if (panelY + 220 > window.innerHeight) {
                    panelY = startCoords.top - 230;
                }
            } catch (_) { /* coordsAtPos may fail at edge positions */ }
        }

        setAiPanel({ action: actionLabel, text: '', loading: true, selectionFrom: from, selectionTo: to, x: panelX, y: panelY });

        const eventId = `editor-ai-${Date.now()}`;
        const unlisten = await listen<{ text: string; isDone: boolean }>(`stream-chunk-${eventId}`, (event) => {
            setAiPanel(prev => prev ? {
                ...prev,
                text: prev.text + event.payload.text,
                loading: !event.payload.isDone,
            } : null);
            if (event.payload.isDone) {
                aiUnlistenRef.current = null;
                unlisten();
            }
        });
        aiUnlistenRef.current = unlisten;

        try {
            await invoke('stream_completion_cmd', { systemPrompt, prompt: selectedText, eventId });
        } catch (error) {
            unlisten();
            aiUnlistenRef.current = null;
            setAiPanel(prev => prev ? { ...prev, loading: false, text: `錯誤：${String(error)}` } : null);
        }
    };

    // ── 檔案選單 ──
    const handleNewNote = () => {
        setShowFileMenu(false);
        const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
        openEditor(`notes/note-${ts}.md`);
    };

    const handleOpenFile = async () => {
        setShowFileMenu(false);
        const { open } = await import('@tauri-apps/plugin-dialog');
        const settings = await tauriCmd.getSettings();
        const kbPath = settings.knowledge.kbPath.replace(/\\/g, '/');
        const selected = await open({
            defaultPath: `${kbPath}/notes`,
            filters: [{ name: 'Markdown', extensions: ['md'] }],
            multiple: false,
        });
        if (!selected || typeof selected !== 'string') return;
        const rel = selected.replace(/\\/g, '/');
        openEditor(rel.startsWith(kbPath + '/') ? rel.slice(kbPath.length + 1) : rel);
    };

    // base64 data URL → ./assets/ 相對路徑，用於所有儲存路徑
    const getMarkdownForSave = useCallback((): string => {
        if (!editor) return '';
        let md = (editor.storage as any).markdown.getMarkdown();
        for (const [dataUrl, relPath] of imgAssetMap.current) {
            md = md.split(dataUrl).join(relPath);
        }
        return md;
    }, [editor]);

    const handleManualSave = async () => {
        if (!editor) return;
        setShowFileMenu(false);
        onSaveStatus?.('saving');
        try {
            const markdown = getMarkdownForSave();
            await tauriCmd.saveDocument(filePath, markdown);
            saveSnapshot(filePath, markdown, editor.state.doc.textContent.length);
            setHistoryTick(t => t + 1);
            onSaveStatus?.('saved');
            setSavePathHint(filePath);
        } catch (e) {
            console.error('[EditorPane] manual save failed:', e);
            onSaveStatus?.('error');
        }
    };

    const handleCloseTab = () => {
        setShowFileMenu(false);
        closeEditorTab(filePath);
    };

    // ── 標題下拉 ──
    const getHeadingLabel = () => {
        if (!editor) return T('editor.bodyText');
        for (let i = 1; i <= 6; i++) {
            if (editor.isActive('heading', { level: i })) return `${T('editor.heading')} ${i}`;
        }
        return T('editor.bodyText');
    };

    // ── Phase B: 字型大小 ──
    const handleFontSizeChange = (size: string) => {
        if (!editor) return;
        if (!size) {
            (editor.chain().focus() as any).unsetFontSize().run();
        } else {
            (editor.chain().focus() as any).setFontSize(`${size}pt`).run();
        }
    };

    const getCurrentFontSize = (): string => {
        if (!editor) return DEFAULT_FONT_SIZE;
        const attrs = editor.getAttributes('textStyle');
        const raw: string = attrs?.fontSize ?? '';
        return raw.replace('pt', '') || DEFAULT_FONT_SIZE;
    };

    // ── Phase B: 字體顏色 ──
    const handleColorChange = (color: string) => {
        if (!editor) return;
        setShowColorPicker(false);
        if (!color) {
            editor.chain().focus().unsetColor().run();
        } else {
            editor.chain().focus().setColor(color).run();
        }
    };

    // ── Phase B: 背景高亮 ──
    const handleHighlightChange = (color: string) => {
        if (!editor) return;
        setShowHighlightPicker(false);
        if (!color) {
            editor.chain().focus().unsetHighlight().run();
        } else {
            editor.chain().focus().setHighlight({ color }).run();
        }
    };

    // ── Phase B: 字型 ──
    const handleFontChange = (fontValue: string) => {
        if (!editor) return;
        if (!fontValue) {
            editor.chain().focus().unsetFontFamily().run();
        } else {
            editor.chain().focus().setFontFamily(fontValue).run();
        }
    };

    const currentFont = FONTS.find(
        f => f.value && editor?.isActive('textStyle', { fontFamily: f.value })
    )?.value ?? '';

    // ── 行距 ──
    const handleLineHeightChange = (value: string) => {
        if (!editor) return;
        if (!value) {
            (editor.chain().focus() as any).unsetLineHeight().run();
        } else {
            (editor.chain().focus() as any).setLineHeight(value).run();
        }
    };

    const getCurrentLineHeight = (): string => {
        if (!editor) return DEFAULT_LINE_SPACING;
        const paraAttrs = editor.getAttributes('paragraph');
        const headingAttrs = editor.getAttributes('heading');
        return paraAttrs?.lineHeight || headingAttrs?.lineHeight || DEFAULT_LINE_SPACING;
    };


    // ── Phase B: 圖片（本地檔案） ──
    const handleInsertImage = async () => {
        if (!editor) return;
        const { open } = await import('@tauri-apps/plugin-dialog');
        const selected = await open({
            filters: [{ name: '圖片', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'] }],
            multiple: false,
        });
        if (!selected || typeof selected !== 'string') return;
        try {
            // 複製到 assets/ 目錄（避免 base64 嵌入 .md）
            const relPath = await tauriCmd.copyImageToAssets(filePath, selected);
            // 讀取 base64 供編輯器即時顯示
            const dataUrl = await tauriCmd.readImageBase64(selected);
            imgAssetMap.current.set(dataUrl, relPath);
            (editor.chain().focus() as any).setImage({ src: dataUrl }).run();
        } catch (e) {
            console.error('[InsertImage]', e);
        }
    };

    // ── Phase B: 連結 ──
    const handleInsertLink = () => {
        if (!editor) return;
        const url = window.prompt(T('editor.linkPrompt'));
        if (!url?.trim()) return;
        if (editor.state.selection.empty) {
            editor.chain().focus().insertContent(`<a href="${url.trim()}">${url.trim()}</a>`).run();
        } else {
            (editor.chain().focus() as any).setLink({ href: url.trim() }).run();
        }
    };

    // ── Phase B: 匯出 ──
    // 計算預設路徑：{kb_path}/{doc_dir}/{exportSubdir}/{stem}_{YYYYMMDD}.{ext}
    const getExportDefaultPath = useCallback(async (ext: string): Promise<string | undefined> => {
        try {
            const settings = await tauriCmd.getSettings();
            const kbPath = settings.knowledge.kbPath.replace(/[/\\]+$/, '');
            const subdir = settings.editor?.exportSubdir?.trim() || 'exports';
            const parts = filePath.split('/');
            const stem = (parts.pop() ?? 'note').replace(/\.md$/, '');
            const dir = parts.join('/');
            const date = new Date().toISOString().slice(0, 10).replace(/-/g, '');
            const docDir = dir ? `${kbPath}/${dir}` : kbPath;
            return subdir ? `${docDir}/${subdir}/${stem}_${date}.${ext}` : `${docDir}/${stem}_${date}.${ext}`;
        } catch {
            return undefined;
        }
    }, [filePath]);

    // 切換檔案時清除 Ctrl+S 記憶的路徑
    useEffect(() => {
        localSavePathRef.current = null;
    }, [filePath]);

    // 儲存路徑提示：3 秒後自動消失
    useEffect(() => {
        if (!savePathHint) return;
        const t = setTimeout(() => setSavePathHint(null), 3000);
        return () => clearTimeout(t);
    }, [savePathHint]);

    // ── 共用：HTML → DOCX base64（含表格邊框樣式）────────────────────────────
    const htmlToDocxBase64 = useCallback(async (html: string): Promise<string> => {
        const wrapped = `<html><head><style>
            table { border-collapse: collapse; width: 100%; }
            table, th, td { border: 1px solid #000; }
            th, td { padding: 4pt 6pt; }
        </style></head><body>${html}</body></html>`;
        const blob = await asBlob(
            wrapped,
            { orientation: 'portrait', margins: { top: 720, right: 720, bottom: 720, left: 720 } }
        ) as Blob;
        const arrayBuffer = await blob.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);
        let binary = '';
        for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
        return btoa(binary);
    }, []);

    // ── Ctrl+S 本地儲存 ──────────────────────────────────────────────────────
    const handleCtrlSave = useCallback(async () => {
        if (!editor) return;

        let savePath = localSavePathRef.current;

        if (!savePath) {
            // 首次：依設定決定預設格式並彈出儲存對話框
            const settings = await tauriCmd.getSettings().catch(() => null);
            const fmt = settings?.editor?.defaultExportFormat ?? 'docx';
            const defaultPath = await getExportDefaultPath(fmt)
                ?? filePath.split('/').pop()?.replace(/\.md$/, `.${fmt}`) ?? `note.${fmt}`;

            const allFilters = [
                { name: 'Word Document', extensions: ['docx'] },
                { name: 'Markdown', extensions: ['md'] },
                { name: 'Text File', extensions: ['txt'] },
            ];
            // 將預設格式排到第一個（Tauri dialog 預設選第一個 filter）
            const filters = [
                ...allFilters.filter(f => f.extensions[0] === fmt),
                ...allFilters.filter(f => f.extensions[0] !== fmt),
            ];

            const chosen = await save({ filters, defaultPath });
            if (!chosen) return;
            savePath = chosen;
            localSavePathRef.current = chosen;
        }

        const ext = savePath.split('.').pop()?.toLowerCase();
        try {
            if (ext === 'docx') {
                const base64 = await htmlToDocxBase64(editor.getHTML());
                await tauriCmd.writeBinaryFile(savePath, base64);
            } else if (ext === 'md') {
                await tauriCmd.exportDocument(savePath, getMarkdownForSave());
            } else {
                await tauriCmd.exportDocument(savePath, editor.getText());
            }
            onSaveStatus?.('saved');
        } catch (e) {
            console.error('[Ctrl+S]', e);
            onSaveStatus?.('error');
        }
    }, [editor, filePath, getExportDefaultPath, onSaveStatus, htmlToDocxBase64]);

    // 監聽 Ctrl+S
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 's' && !e.shiftKey) {
                e.preventDefault();
                handleCtrlSave();
            }
        };
        window.addEventListener('keydown', handler);
        return () => window.removeEventListener('keydown', handler);
    }, [handleCtrlSave]);

    const handlePublish = async () => {
        if (!editor) return;
        setShowFileMenu(false);
        onSaveStatus?.('saving');
        try {
            const markdown = getMarkdownForSave();
            const title = filePath.split('/').pop()?.replace(/\.md$/, '') || 'Untitled';
            await tauriCmd.saveEditorToKnowledge(title, markdown);
            // alert('已成功發佈至知識庫並自動分段入庫！'); // 可關閉 alert 或使用 Toast
            onSaveStatus?.('saved');
            setSavePathHint("已發佈至知識庫");
        } catch (e) {
            console.error('[Publish]', e);
            alert(`發佈失敗：${e}`);
            onSaveStatus?.('error');
        }
    };

    const handleExportTxt = async () => {
        if (!editor) return;
        setShowExport(false);
        const text = editor.getText();
        const defaultPath = await getExportDefaultPath('txt')
            ?? filePath.split('/').pop()?.replace(/\.md$/, '.txt') ?? 'note.txt';
        const path = await save({
            filters: [{ name: 'Text File', extensions: ['txt'] }],
            defaultPath,
        });
        if (!path) return;
        try {
            await tauriCmd.exportDocument(path, text);
        } catch (e) {
            console.error('[Export TXT]', e);
        }
    };

    const handleExportMd = async () => {
        if (!editor) return;
        setShowExport(false);
        const md = getMarkdownForSave();
        const defaultPath = await getExportDefaultPath('md')
            ?? filePath.split('/').pop() ?? 'note.md';
        const path = await save({
            filters: [{ name: 'Markdown', extensions: ['md'] }],
            defaultPath,
        });
        if (!path) return;
        try {
            await tauriCmd.exportDocument(path, md);
        } catch (e) {
            console.error('[Export MD]', e);
        }
    };

    const handleExportDocx = async () => {
        if (!editor) return;
        setShowExport(false);
        const html = editor.getHTML();
        const defaultPath = await getExportDefaultPath('docx')
            ?? filePath.split('/').pop()?.replace(/\.md$/, '.docx') ?? 'note.docx';
        const path = await save({
            filters: [{ name: 'Word Document', extensions: ['docx'] }],
            defaultPath,
        });
        if (!path) return;
        try {
            const base64 = await htmlToDocxBase64(html);
            await tauriCmd.writeBinaryFile(path, base64);
        } catch (e) {
            console.error('[Export DOCX]', e);
            alert(`匯出 DOCX 失敗：${e}`);
        }
    };

    const handleExportXlsx = async () => {
        if (!editor) return;
        setShowExport(false);
        const md = (editor.storage as any).markdown.getMarkdown();
        const defaultPath = await getExportDefaultPath('xlsx')
            ?? filePath.split('/').pop()?.replace(/\.md$/, '.xlsx') ?? 'note.xlsx';
        const path = await save({
            filters: [{ name: 'Excel Workbook', extensions: ['xlsx'] }],
            defaultPath,
        });
        if (!path) return;
        try {
            await tauriCmd.exportXlsx(path, md);
        } catch (e) {
            console.error('[Export XLSX]', e);
        }
    };

    const handleExportPdf = () => {
        setShowExport(false);
        // 注入 @media print 樣式，只顯示編輯器內容區
        const styleId = '__editor_print_style__';
        let styleEl = document.getElementById(styleId) as HTMLStyleElement | null;
        if (!styleEl) {
            styleEl = document.createElement('style');
            styleEl.id = styleId;
            document.head.appendChild(styleEl);
        }
        styleEl.textContent = `
            @media print {
                body > * { display: none !important; }
                #editor-print-root { display: flex !important; position: fixed !important;
                    inset: 0 !important; width: 100% !important; height: auto !important; }
                #editor-print-root .no-print { display: none !important; }
            }
        `;
        window.print();
    };

    if (!editor) {
        return <div className="p-4 text-center text-ic-text-muted">{T('editor.loading')}</div>;
    }

    return (
        <div
            id="editor-print-root"
            className="flex flex-col h-full bg-ic-bg-surface overflow-hidden shadow-sm rounded-lg border border-ic-border"
        >
            {/* Toolbar — 排列順序依 EDITOR_MODULE.md §2.2 */}
            <div className="no-print bg-ic-bg-elevated border-b border-ic-border px-3 py-2 flex items-center gap-1 flex-wrap select-none">

                {/* ── 1. 檔案選單 ── */}
                <div className="relative">
                    <button
                        onMouseDown={e => { e.preventDefault(); e.stopPropagation(); setShowHeadingMenu(false); setShowListMenu(false); setShowTableMenu(false); setShowFileMenu(v => !v); }}
                        className="px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                    >
                        {T('editor.file')}
                    </button>
                    {showFileMenu && (
                        <div
                            onMouseDown={e => e.stopPropagation()}
                            className="absolute top-full left-0 mt-1 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-50 min-w-[160px] py-1 px-1 select-none"
                        >
                            <MenuBtn onClick={handleNewNote} label={T('editor.new')} />
                            <MenuBtn onClick={handleOpenFile} label={T('editor.open')} />
                            <MenuBtn onClick={handleManualSave} label={T('editor.save')} />
                            <div className="h-px bg-ic-border my-0.5" />
                            <button
                                onClick={handlePublish}
                                className="w-full text-left px-3 py-1.5 text-xs font-medium text-emerald-600 dark:text-emerald-400 hover:bg-ic-bg-hover rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap"
                            >
                                🚀 發佈至知識庫
                            </button>
                            <div className="h-px bg-ic-border my-0.5" />
                            <div className="px-3 py-1 text-[10px] font-semibold text-ic-text-muted uppercase tracking-wider">{T('editor.saveAs')}</div>
                            <button onClick={handleExportTxt} className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap">
                                <FileText className="w-3 h-3" />Plain Text (.txt)
                            </button>
                            <button onClick={handleExportMd} className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap">
                                <FileCode className="w-3 h-3" />Markdown (.md)
                            </button>
                            <button onClick={handleExportDocx} className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap">
                                <FileText className="w-3 h-3" />Word (.docx)
                            </button>
                            <button onClick={handleExportXlsx} className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap">
                                <Grid3x3 className="w-3 h-3" />Excel (.xlsx)
                            </button>
                            <button onClick={handleExportPdf} className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap">
                                <Printer className="w-3 h-3" />PDF ({T('editor.print')})
                            </button>
                            <div className="h-px bg-ic-border my-0.5" />
                            <MenuBtn onClick={handleCloseTab} label={T('editor.close')} />
                        </div>
                    )}
                </div>

                <Divider />

                {/* ── 2. B I U S ── */}
                <ToolbarBtn active={editor.isActive('bold')} onClick={() => editor.chain().focus().toggleBold().run()} label="B" className="font-bold" />
                <ToolbarBtn active={editor.isActive('italic')} onClick={() => editor.chain().focus().toggleItalic().run()} label="I" className="italic" />
                <button
                    onMouseDown={e => { e.preventDefault(); (editor.chain().focus() as any).toggleUnderline().run(); }}
                    title={T('editor.underline')}
                    className={`px-2 py-1 text-xs rounded select-none transition-colors underline
                        ${editor.isActive('underline')
                            ? 'bg-ic-bg-active text-ic-text-primary'
                            : 'text-ic-text-secondary hover:bg-ic-bg-hover'}`}
                >
                    <UnderlineIcon className="w-3.5 h-3.5" />
                </button>
                <ToolbarBtn active={editor.isActive('strike')} onClick={() => editor.chain().focus().toggleStrike().run()} label="S" className="line-through" />

                <Divider />

                {/* ── 3. 字型 大小 標題 ── */}
                <select
                    value={currentFont}
                    onChange={e => handleFontChange(e.target.value)}
                    className="text-xs px-1.5 py-1 rounded bg-ic-bg-elevated border border-ic-border text-ic-text-secondary hover:border-ic-border-strong focus:outline-none cursor-pointer"
                    title={T('editor.font')}
                >
                    {FONTS.map(f => (
                        <option key={f.value} value={f.value}>{f.value === '' ? T('editor.defaultFont') : f.label}</option>
                    ))}
                </select>
                <select
                    value={getCurrentFontSize()}
                    onChange={e => handleFontSizeChange(e.target.value)}
                    className="text-xs px-1 py-1 w-14 rounded bg-ic-bg-elevated border border-ic-border text-ic-text-secondary hover:border-ic-border-strong focus:outline-none cursor-pointer"
                    title={T('editor.fontSize')}
                >
                    {FONT_SIZES.map(s => (
                        <option key={s} value={s}>{s}</option>
                    ))}
                </select>
                <div className="relative">
                    <button
                        onMouseDown={e => { e.preventDefault(); e.stopPropagation(); setShowFileMenu(false); setShowListMenu(false); setShowTableMenu(false); setShowHeadingMenu(v => !v); }}
                        className="flex items-center gap-0.5 px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                    >
                        {getHeadingLabel()}<ChevronDown className="w-3 h-3" />
                    </button>
                    {showHeadingMenu && (
                        <div
                            onMouseDown={e => e.stopPropagation()}
                            className="absolute top-full left-0 mt-1 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-50 min-w-[110px] py-1 px-1 select-none"
                        >
                            {HEADING_ITEMS.map(item => (
                                <button
                                    key={item.level}
                                    onClick={() => {
                                        if (item.level === 0) {
                                            editor.chain().focus().setParagraph().run();
                                        } else {
                                            editor.chain().focus().toggleHeading({ level: item.level as any }).run();
                                        }
                                        setShowHeadingMenu(false);
                                    }}
                                    className={`w-full text-left px-3 py-1.5 text-xs hover:bg-ic-bg-hover transition-colors
                                        ${item.level === 0 ? 'text-ic-text-secondary' : ''}
                                        ${item.level === 1 ? 'text-base font-bold text-ic-text-primary' : ''}
                                        ${item.level === 2 ? 'text-sm font-semibold text-ic-text-secondary' : ''}
                                        ${item.level >= 3 ? 'text-xs text-ic-text-muted' : ''}`}
                                >
                                    {item.label}
                                </button>
                            ))}
                        </div>
                    )}
                </div>

                <Divider />

                {/* ── 4. 字體顏色 高亮 ── */}
                <div className="relative">
                    <button
                        onMouseDown={e => { e.preventDefault(); e.stopPropagation(); setShowHighlightPicker(false); setShowExport(false); setShowColorPicker(v => !v); }}
                        title={T('editor.fontColor')}
                        className="px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors flex flex-col items-center gap-0.5"
                    >
                        <Baseline className="w-3.5 h-3.5" />
                        <div
                            className="w-3.5 h-1 rounded-sm"
                            style={{ backgroundColor: editor?.getAttributes('textStyle')?.color || '#000000' }}
                        />
                    </button>
                    {showColorPicker && (
                        <div
                            onMouseDown={e => e.stopPropagation()}
                            className="absolute top-full left-0 mt-1 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-50 p-2 select-none"
                        >
                            <div className="flex flex-wrap gap-1 w-32">
                                {TEXT_COLORS.map(c => (
                                    <button
                                        key={c.value}
                                        onClick={() => handleColorChange(c.value)}
                                        title={c.label}
                                        className="w-6 h-6 rounded border border-ic-border hover:scale-110 transition-transform"
                                        style={{ backgroundColor: c.value || 'transparent' }}
                                    >
                                        {!c.value && <span className="text-xs text-ic-text-muted">✕</span>}
                                    </button>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
                <div className="relative">
                    <button
                        onMouseDown={e => { e.preventDefault(); e.stopPropagation(); setShowColorPicker(false); setShowExport(false); setShowHighlightPicker(v => !v); }}
                        title={T('editor.highlight')}
                        className="px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                    >
                        <Highlighter className="w-3.5 h-3.5" />
                    </button>
                    {showHighlightPicker && (
                        <div
                            onMouseDown={e => e.stopPropagation()}
                            className="absolute top-full left-0 mt-1 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-50 p-2 select-none"
                        >
                            <div className="flex flex-wrap gap-1 w-28">
                                {HIGHLIGHT_COLORS.map(c => (
                                    <button
                                        key={c.value}
                                        onClick={() => handleHighlightChange(c.value)}
                                        title={c.label}
                                        className="w-6 h-6 rounded border border-ic-border hover:scale-110 transition-transform"
                                        style={{ backgroundColor: c.value || 'transparent' }}
                                    >
                                        {!c.value && <span className="text-xs text-ic-text-muted">✕</span>}
                                    </button>
                                ))}
                            </div>
                        </div>
                    )}
                </div>

                <Divider />

                {/* ── 5. 對齊 ── */}
                {(['left', 'center', 'right', 'justify'] as const).map((align, i) => {
                    const icons = [AlignLeft, AlignCenter, AlignRight, AlignJustify];
                    const labels = [T('editor.alignLeft'), T('editor.alignCenter'), T('editor.alignRight'), T('editor.alignJustify')];
                    const Icon = icons[i];
                    return (
                        <button
                            key={align}
                            onMouseDown={e => { e.preventDefault(); (editor.chain().focus() as any).setTextAlign(align).run(); }}
                            title={labels[i]}
                            className={`px-2 py-1 text-xs rounded select-none transition-colors
                                ${editor.isActive({ textAlign: align })
                                    ? 'bg-ic-bg-active text-ic-text-primary'
                                    : 'text-ic-text-secondary hover:bg-ic-bg-hover'}`}
                        >
                            <Icon className="w-3.5 h-3.5" />
                        </button>
                    );
                })}

                <Divider />

                {/* ── 6. 間距 ── */}
                <select
                    value={getCurrentLineHeight()}
                    onChange={e => handleLineHeightChange(e.target.value)}
                    className="text-xs px-1 py-1 w-20 rounded bg-ic-bg-elevated border border-ic-border text-ic-text-secondary hover:border-ic-border-strong focus:outline-none cursor-pointer"
                    title={T('editor.lineSpacing')}
                >
                    {LINE_SPACINGS.map(s => (
                        <option key={s.value} value={s.value}>{s.label}</option>
                    ))}
                </select>

                <Divider />

                {/* ── 7. 清單 ── */}
                <div className="relative">
                    <button
                        onMouseDown={e => { e.preventDefault(); e.stopPropagation(); setShowFileMenu(false); setShowHeadingMenu(false); setShowTableMenu(false); setShowListMenu(v => !v); }}
                        className="flex items-center gap-0.5 px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                    >
                        {T('editor.list')}<ChevronDown className="w-3 h-3" />
                    </button>
                    {showListMenu && (
                        <div
                            onMouseDown={e => e.stopPropagation()}
                            className="absolute top-full left-0 mt-1 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-50 min-w-[130px] py-1 px-1 select-none"
                        >
                            <MenuBtn onClick={() => {
                                // 若已在編號清單中，先縮排為子層再切換為符號清單（建立巢狀）
                                if (editor.isActive('orderedList')) {
                                    editor.chain().focus().sinkListItem('listItem').toggleBulletList().run();
                                } else {
                                    editor.chain().focus().toggleBulletList().run();
                                }
                                setShowListMenu(false);
                            }} label={T('editor.bulletList')} />
                            <MenuBtn onClick={() => {
                                // 若已在符號清單中，先縮排為子層再切換為編號清單（建立巢狀）
                                if (editor.isActive('bulletList')) {
                                    editor.chain().focus().sinkListItem('listItem').toggleOrderedList().run();
                                } else {
                                    editor.chain().focus().toggleOrderedList().run();
                                }
                                setShowListMenu(false);
                            }} label={T('editor.orderedList')} />
                            <div className="h-px bg-ic-border my-0.5" />
                            <MenuBtn onClick={() => { (editor.chain().focus() as any).sinkListItem('listItem').run(); setShowListMenu(false); }} label={T('editor.indent')} />
                            <MenuBtn onClick={() => { (editor.chain().focus() as any).liftListItem('listItem').run(); setShowListMenu(false); }} label={T('editor.outdent')} />
                        </div>
                    )}
                </div>

                <Divider />

                {/* ── 8. 引用 分隔線 程式碼 ── */}
                <ToolbarBtn active={editor.isActive('blockquote')} onClick={() => editor.chain().focus().toggleBlockquote().run()} label="❝" />
                <button
                    onMouseDown={e => { e.preventDefault(); editor.chain().focus().setHorizontalRule().run(); }}
                    title={T('editor.insertHr')}
                    className="px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                >
                    <Minus className="w-3.5 h-3.5" />
                </button>
                <ToolbarBtn active={editor.isActive('codeBlock')} onClick={() => editor.chain().focus().toggleCodeBlock().run()} label="<>" className="font-mono" />

                <Divider />

                {/* ── 9. 圖片 表格 連結 ── */}
                <button
                    onMouseDown={e => { e.preventDefault(); handleInsertImage(); }}
                    title={T('editor.insertImage')}
                    className="px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                >
                    <ImagePlus className="w-3.5 h-3.5" />
                </button>
                <div className="relative">
                    <button
                        onMouseDown={e => { e.preventDefault(); e.stopPropagation(); setShowFileMenu(false); setShowHeadingMenu(false); setShowListMenu(false); setShowTableMenu(v => !v); }}
                        className="flex items-center gap-0.5 px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                        title={T('editor.table')}
                    >
                        <Grid3x3 className="w-3.5 h-3.5" />
                        <ChevronDown className="w-3 h-3" />
                    </button>
                    {showTableMenu && (
                        <div
                            onMouseDown={e => e.stopPropagation()}
                            className="absolute top-full left-0 mt-1 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-50 p-2 min-w-[200px] select-none"
                        >
                            <div className="text-xs text-center text-ic-text-muted mb-1.5 h-4">
                                {tableGridHover
                                    ? T('editor.tableGridDisplay').replace('{row}', String(tableGridHover.row + 1)).replace('{col}', String(tableGridHover.col + 1))
                                    : T('editor.tableSelectSize')}
                            </div>
                            <div className="grid grid-cols-5 gap-0.5 mb-2" onMouseLeave={() => setTableGridHover(null)}>
                                {Array.from({ length: 5 }, (_, r) =>
                                    Array.from({ length: 5 }, (_, c) => (
                                        <div
                                            key={`${r}-${c}`}
                                            onMouseEnter={() => setTableGridHover({ row: r, col: c })}
                                            onClick={() => {
                                                editor.chain().focus().insertTable({ rows: r + 1, cols: c + 1, withHeaderRow: false }).run();
                                                setShowTableMenu(false);
                                                setTableGridHover(null);
                                            }}
                                            className={`w-6 h-6 border rounded-sm cursor-pointer transition-colors
                                                ${tableGridHover && r <= tableGridHover.row && c <= tableGridHover.col
                                                    ? 'bg-emerald-100 dark:bg-ic-accent/40 border-ic-accent dark:border-ic-accent'
                                                    : 'border-ic-border hover:border-ic-border-strong'}`}
                                        />
                                    ))
                                )}
                            </div>
                            <div className="border-t border-ic-border mb-1" />
                            <button
                                onClick={() => {
                                    const rows = tableGridHover ? tableGridHover.row + 1 : 3;
                                    const cols = tableGridHover ? tableGridHover.col + 1 : 3;
                                    editor.chain().focus().insertTable({ rows, cols, withHeaderRow: true }).run();
                                    setShowTableMenu(false);
                                    setTableGridHover(null);
                                }}
                                className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors"
                            >
                                {T('editor.insertTable')}
                            </button>
                            {editor.isActive('table') && (
                                <>
                                    <div className="border-t border-ic-border my-1" />
                                    <button
                                        onClick={() => { editor.chain().focus().deleteTable().run(); setShowTableMenu(false); }}
                                        className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-destructive hover:text-ic-destructive hover:bg-ic-destructive-subtle rounded-lg transition-colors"
                                    >
                                        {T('editor.ctx.deleteTable')}
                                    </button>
                                    <div className="border-t border-ic-border my-1" />
                                    <div className="group/tblEdit relative">
                                        <div className="flex items-center justify-between px-2 py-1.5 text-xs text-ic-text-secondary hover:bg-ic-bg-hover rounded cursor-default transition-colors">
                                            {T('editor.editTable')}<ChevronRight className="w-3 h-3" />
                                        </div>
                                        <div className="hidden group-hover/tblEdit:block absolute left-full top-0 ml-0.5 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-[60] min-w-[90px] py-1 px-1 select-none">
                                            <div className="group/tblIns relative">
                                                <div className="flex items-center justify-between px-3 py-1.5 text-xs text-ic-text-secondary hover:bg-ic-bg-hover cursor-default">
                                                    {T('editor.tableInsert')}<ChevronRight className="w-3 h-3 ml-2" />
                                                </div>
                                                <div className="hidden group-hover/tblIns:block absolute left-full top-0 ml-0.5 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-[70] min-w-[100px] py-1 px-1">
                                                    <MenuBtn onClick={() => { editor.chain().focus().addColumnBefore().run(); setShowTableMenu(false); }} label={T('editor.tableAddColLeft')} />
                                                    <MenuBtn onClick={() => { editor.chain().focus().addColumnAfter().run(); setShowTableMenu(false); }} label={T('editor.tableAddColRight')} />
                                                    <MenuBtn onClick={() => { editor.chain().focus().addRowBefore().run(); setShowTableMenu(false); }} label={T('editor.tableAddRowAbove')} />
                                                    <MenuBtn onClick={() => { editor.chain().focus().addRowAfter().run(); setShowTableMenu(false); }} label={T('editor.tableAddRowBelow')} />
                                                </div>
                                            </div>
                                            <div className="group/tblDel relative">
                                                <div className="flex items-center justify-between px-3 py-1.5 text-xs text-ic-text-secondary hover:bg-ic-bg-hover cursor-default">
                                                    {T('editor.tableDelete')}<ChevronRight className="w-3 h-3 ml-2" />
                                                </div>
                                                <div className="hidden group-hover/tblDel:block absolute left-full top-0 ml-0.5 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-lg z-[70] min-w-[90px] py-1 px-1">
                                                    <MenuBtn onClick={() => { editor.chain().focus().deleteTable().run(); setShowTableMenu(false); }} label={T('editor.tableDelTable')} />
                                                    <MenuBtn onClick={() => { editor.chain().focus().deleteColumn().run(); setShowTableMenu(false); }} label={T('editor.tableDelCol')} />
                                                    <MenuBtn onClick={() => { editor.chain().focus().deleteRow().run(); setShowTableMenu(false); }} label={T('editor.tableDelRow')} />
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </>
                            )}
                        </div>
                    )}
                </div>
                <button
                    onMouseDown={e => { e.preventDefault(); handleInsertLink(); }}
                    title={T('editor.insertLink')}
                    className="px-2 py-1 text-xs rounded text-ic-text-secondary hover:bg-ic-bg-hover transition-colors"
                >
                    <Link2 className="w-3.5 h-3.5" />
                </button>

                <Divider />

                {/* ── 10. 尋找 ── */}
                <button
                    onMouseDown={e => { e.preventDefault(); setSearchMode(prev => prev ? null : 'find'); }}
                    title={T('editor.findReplace')}
                    className={`px-2 py-1 text-xs rounded transition-colors flex items-center gap-1
                        ${searchMode
                            ? 'bg-ic-info/20 text-ic-info'
                            : 'text-ic-text-secondary hover:bg-ic-bg-hover'}`}
                >
                    <Search className="w-3.5 h-3.5" />
                </button>
            </div>

            {/* Editor Content + History Panel */}
            <div className="flex flex-1 overflow-hidden">
                <div
                    className={`flex-1 overflow-y-auto px-8 py-6 relative transition-colors ${isDragOver ? 'ring-2 ring-inset ring-ic-accent/50 bg-ic-accent/5' : ''}`}
                    data-editor-drop="true"
                    data-editor-area="true"
                >
                    <EditorContent editor={editor} className="min-h-full" onContextMenu={handleEditorContextMenu} />

                    {/* 尋找/取代 浮動列 */}
                    {searchMode && (
                        <SearchReplaceBar
                            editor={editor}
                            mode={searchMode}
                            searchTerm={searchTerm}
                            replaceTerm={replaceTerm}
                            onSearchChange={setSearchTerm}
                            onReplaceChange={setReplaceTerm}
                            onClose={() => setSearchMode(null)}
                        />
                    )}

                    {/* 編輯器右鍵選單（AI 功能 + 標準編輯 + 表格操作） */}
                    {editorContextMenu && editor && (
                        <EditorContextMenu
                            x={editorContextMenu.x}
                            y={editorContextMenu.y}
                            editor={editor}
                            inTable={editorContextMenu.inTable}
                            selectedText={editorContextMenu.selectedText}
                            onClose={() => setEditorContextMenu(null)}
                            onAiAction={(action) => {
                                if (!editorContextMenu) return;
                                handleAiAction(action, editorContextMenu.selectedText, editorContextMenu.selectionFrom, editorContextMenu.selectionTo);
                                setEditorContextMenu(null);
                            }}
                        />
                    )}

                    {/* AI 生成結果面板 */}
                    {aiPanel && (
                        <div
                            style={{ position: 'fixed', left: aiPanel.x, top: aiPanel.y, maxWidth: 'min(500px, 92vw)' }}
                            className="z-50 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-2xl p-4 w-[500px]"
                        >
                            <div className="flex items-center justify-between mb-2">
                                <span className="text-xs font-semibold text-purple-600 dark:text-purple-400">{aiPanel.action}</span>
                                {aiPanel.loading && (
                                    <span className="text-[11px] text-ic-text-muted animate-pulse">{T('editor.aiGenerating')}</span>
                                )}
                            </div>
                            <div className="text-sm text-ic-text-secondary max-h-44 overflow-y-auto bg-ic-bg-surface rounded-lg p-3 mb-3 leading-relaxed whitespace-pre-wrap">
                                {aiPanel.text || <span className="text-ic-text-muted italic text-xs">{T('editor.aiWaiting')}</span>}
                            </div>
                            <div className="flex gap-2 justify-end">
                                <button
                                    onClick={() => {
                                        aiUnlistenRef.current?.();
                                        setAiPanel(null);
                                    }}
                                    className="px-3 py-1.5 text-xs text-ic-text-secondary hover:bg-ic-bg-hover rounded-lg transition-colors"
                                >
                                    {T('editor.aiCancel')}
                                </button>
                                {!aiPanel.loading && aiTaskRef.current && (
                                    <button
                                        onClick={() => {
                                            const t = aiTaskRef.current!;
                                            handleAiAction(t.action, t.selectedText, t.from, t.to);
                                        }}
                                        className="px-3 py-1.5 text-xs text-ic-info hover:bg-ic-info/10 rounded-lg transition-colors"
                                    >
                                        {T('editor.aiRetry')}
                                    </button>
                                )}
                                {!aiPanel.loading && aiPanel.text && editor && (
                                    <button
                                        onClick={() => {
                                            editor.chain()
                                                .focus()
                                                .setTextSelection({ from: aiPanel.selectionFrom, to: aiPanel.selectionTo })
                                                .insertContent(aiPanel.text)
                                                .run();
                                            setAiPanel(null);
                                        }}
                                        className="px-3 py-1.5 text-xs bg-purple-600 hover:bg-purple-700 text-white rounded-lg transition-colors"
                                    >
                                        {T('editor.aiApply')}
                                    </button>
                                )}
                            </div>
                        </div>
                    )}
                </div>

                {/* Phase C: 版本歷史面板 */}
                {showHistory && (
                    <div className="w-52 shrink-0 border-l border-ic-border flex flex-col bg-ic-bg-elevated">
                        <div className="flex items-center justify-between px-3 py-2 border-b border-ic-border">
                            <span className="text-xs font-medium text-ic-text-secondary">{T('editor.versionHistory')}</span>
                            <button
                                onMouseDown={e => { e.preventDefault(); setShowHistory(false); }}
                                className="p-1 rounded hover:bg-ic-bg-hover"
                            >
                                <XIcon className="w-3 h-3 text-ic-text-muted" />
                            </button>
                        </div>
                        <div className="flex-1 overflow-y-auto custom-scrollbar">
                            {/* historyTick forces re-read from localStorage */}
                            {historyTick >= 0 && getSnapshots(filePath).length === 0 ? (
                                <p className="text-xs text-ic-text-muted p-3 text-center">{T('editor.noHistory')}<br /><span className="text-ic-text-muted">{T('editor.historyHint')}</span></p>
                            ) : (
                                getSnapshots(filePath).map((snap, i) => (
                                    <button
                                        key={i}
                                        className="w-full text-left px-3 py-2 border-b border-ic-border/50 hover:bg-ic-bg-hover transition-colors"
                                        onClick={() => {
                                            if (editor) {
                                                editor.commands.setContent(snap.content);
                                                setShowHistory(false);
                                            }
                                        }}
                                    >
                                        <div className="text-xs text-ic-text-secondary">
                                            {new Date(snap.timestamp).toLocaleString('zh-TW', {
                                                month: '2-digit', day: '2-digit',
                                                hour: '2-digit', minute: '2-digit',
                                            })}
                                        </div>
                                        <div className="text-xs text-ic-text-muted mt-0.5">{snap.wordCount} {T('editor.words')}</div>
                                    </button>
                                ))
                            )}
                        </div>
                    </div>
                )}
            </div>

            {/* 狀態列：字數統計 + 歷史 */}
            <div className="no-print flex items-center justify-between gap-4 px-4 py-1 text-xs text-ic-text-muted border-t border-ic-border bg-ic-bg-elevated select-none">
                <button
                    onMouseDown={e => { e.preventDefault(); setShowHistory(v => !v); }}
                    title={T('editor.versionHistory')}
                    className={`flex items-center gap-1 px-1.5 py-0.5 rounded transition-colors ${showHistory
                        ? 'bg-ic-info/20 text-ic-info'
                        : 'hover:bg-ic-bg-hover'
                        }`}
                >
                    <Clock className="w-3 h-3" />
                    <span>{T('editor.history')}</span>
                </button>
                <div className="flex items-center gap-3">
                    {savePathHint && (
                        <span
                            title={`${T('editor.savedToKb')}: ${savePathHint}`}
                            className="text-green-500 dark:text-green-400 cursor-default truncate max-w-[200px]"
                        >
                            ✓ {T('editor.saved')}: {savePathHint}
                        </span>
                    )}
                    <span>{wordCount} {T('editor.words')}</span>
                </div>
            </div>

        </div>
    );
}

function Divider() {
    return <div className="w-px h-4 bg-ic-border mx-1 shrink-0" />;
}

function MenuBtn({ onClick, label }: { onClick: () => void; label: string }) {
    return (
        <button
            onClick={onClick}
            className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary hover:text-ic-text-primary hover:bg-ic-bg-hover rounded-lg transition-colors"
        >
            {label}
        </button>
    );
}

function ToolbarBtn({
    active,
    onClick,
    label,
    className = '',
}: {
    active: boolean;
    onClick: () => void;
    label: string;
    className?: string;
}) {
    return (
        <button
            onMouseDown={e => { e.preventDefault(); onClick(); }}
            className={`px-2 py-1 text-xs rounded select-none transition-colors ${className}
                ${active
                    ? 'bg-ic-bg-active text-ic-text-primary'
                    : 'text-ic-text-secondary hover:bg-ic-bg-hover'}`}
        >
            {label}
        </button>
    );
}
