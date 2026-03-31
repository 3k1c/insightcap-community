import React, { useState, useRef, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { tauriCmd } from '../../lib/tauri';
import { useEditor, EditorContent } from '@tiptap/react';
import { BubbleMenu } from '@tiptap/react/menus';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import Highlight from '@tiptap/extension-highlight';
import Underline from '@tiptap/extension-underline';
import TextAlign from '@tiptap/extension-text-align';
import Link from '@tiptap/extension-link';
import { Table, TableRow, TableCell, TableHeader } from '@tiptap/extension-table';
import BubbleMenuExtension from '@tiptap/extension-bubble-menu';
import { Node as TiptapNode, Extension, mergeAttributes, nodeInputRule } from '@tiptap/core';
import { ReactNodeViewRenderer } from '@tiptap/react';
import ImageNodeView from './extensions/ImageNodeView';
// import { ImageNodePro } from './extensions/ImageNodePro';

const inputRegex = /(?:^|\s)(!\[(.+|:?)]\((\S+)(?:(?:\s+)["'](\S+)["'])?\))$/;

const ImageNodePro = TiptapNode.create({
    name: 'imageNodePro',
    group: 'inline',
    inline: true,
    draggable: true,
    selectable: true,
    atom: true,
    addAttributes() {
        return {
            src: { default: null },
            alt: { default: null },
            title: { default: null },
            width: { default: '100%' },
            textAlign: { default: 'center' },
        };
    },
    parseHTML() {
        return [{ tag: 'div[data-type="image-node-pro"]' }];
    },
    renderHTML({ HTMLAttributes }) {
        return ['div', mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, { 'data-type': 'image-node-pro' })];
    },
    addNodeView() {
        return ReactNodeViewRenderer(ImageNodeView as any);
    },
    addCommands() {
        return {
            setImage: options => ({ commands }) => {
                return commands.insertContent({
                    type: this.name,
                    attrs: options,
                });
            },
        };
    },
    addInputRules() {
        return [
            nodeInputRule({
                find: inputRegex,
                type: this.type,
                getAttributes: match => {
                    const [, , alt, src, title] = match;
                    return { src, alt, title };
                },
            }),
        ];
    },
});
import {
    Bold, Italic, Underline as UnderlineIcon, Heading1, Heading2,
    List, ListOrdered,
    Code, Highlighter, X, Table as TableIcon, Plus, Trash2,
    AlignLeft, AlignCenter, AlignRight, AlignJustify,
    Upload, Columns, Merge, Split, LayoutTemplate, ChevronDown, Link as LinkIcon, Image as ImageIcon,
    Sparkles, Wand2, Eraser, Check, RotateCcw, RefreshCw, FileText, Languages, Smile, ChevronRight,
    FolderOpen, History
} from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import {
    loadSession, saveSession, loadContent, saveContent,
    loadFiles, upsertFile, renameFile as renameNoteFile,
    addHistory, loadHistory,
    HistoryEntry,
} from '../../lib/noteStore';
import { useT } from '../../hooks/useT';

interface MenuBarProps {
    editor: any;
    fileName: string;
    onOpenDocument: () => void;
}

const MenuBar = ({ editor, fileName, onOpenDocument }: MenuBarProps) => {
    const t = useT();
    // Force re-render on cursor/selection change so active states stay in sync
    const [, forceUpdate] = useState(0);
    useEffect(() => {
        if (!editor) return;
        const handler = () => forceUpdate(n => n + 1);
        editor.on('selectionUpdate', handler);
        editor.on('transaction', handler);
        return () => {
            editor.off('selectionUpdate', handler);
            editor.off('transaction', handler);
        };
    }, [editor]);
    const [showTableMenu, setShowTableMenu] = useState(false);
    const tableMenuRef = useRef<HTMLDivElement>(null);
    const [showExportMenu, setShowExportMenu] = useState(false);
    const exportMenuRef = useRef<HTMLDivElement>(null);
    const [showTurnIntoMenu, setShowTurnIntoMenu] = useState(false);
    const turnIntoMenuRef = useRef<HTMLDivElement>(null);
    const [showListMenu, setShowListMenu] = useState(false);
    const listMenuRef = useRef<HTMLDivElement>(null);

    // Close menu when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (tableMenuRef.current && !tableMenuRef.current.contains(event.target as Node)) {
                setShowTableMenu(false);
            }
            if (exportMenuRef.current && !exportMenuRef.current.contains(event.target as Node)) {
                setShowExportMenu(false);
            }
            if (turnIntoMenuRef.current && !turnIntoMenuRef.current.contains(event.target as Node)) {
                setShowTurnIntoMenu(false);
            }
            if (listMenuRef.current && !listMenuRef.current.contains(event.target as Node)) {
                setShowListMenu(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    if (!editor) return null;

    const ToolbarButton = ({ onClick, isActive = false, disabled = false, icon: Icon, title, label }: any) => (
        <button
            onClick={onClick}
            disabled={disabled}
            title={title}
            className={`p-1.5 rounded transition-colors flex items-center gap-1.5 ${isActive ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'} ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
        >
            <Icon className="w-4 h-4" />
            {label && <span className="text-fs-xs font-medium">{label}</span>}
        </button>
    );

    const MenuAction = ({ onClick, icon: Icon, label, disabled = false, danger = false }: any) => (
        <button
            onClick={() => {
                onClick();
                setShowTableMenu(false);
            }}
            disabled={disabled}
            className={`w-full flex items-center gap-3 px-3 py-2 text-fs-sm transition-colors rounded-md ${danger
                ? 'text-status-error hover:bg-status-error/10'
                : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'
                } ${disabled ? 'opacity-40 cursor-not-allowed' : ''}`}
        >
            <Icon className="w-4 h-4 shrink-0" />
            <span>{label}</span>
        </button>
    );

    const exportAs = async (format: 'txt' | 'md' | 'html' | 'docx' | 'pdf') => {
        setShowExportMenu(false);
        const name = fileName || '文件';
        const html = editor.getHTML();
        const text = editor.getText();

        const buildStandaloneHtml = (innerHtml: string) => {
            const minimalCSS = [
                'body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"Helvetica Neue",Arial;color:#111827;padding:32px 40px;max-width:860px;margin:0 auto;background:#ffffff}',
                'h1{font-size:1.5rem;line-height:2rem;font-weight:700;margin-top:1.5rem;margin-bottom:0.75rem;color:#0f172a}',
                'h2{font-size:1.25rem;line-height:1.75rem;font-weight:700;margin-top:1.25rem;margin-bottom:0.5rem;color:#0f172a}',
                'h3{font-size:1.125rem;line-height:1.625rem;font-weight:700;margin-top:1rem;margin-bottom:0.4rem;color:#0f172a}',
                'h4{font-size:1rem;line-height:1.5rem;font-weight:700;margin-top:0.875rem;margin-bottom:0.35rem;color:#0f172a}',
                'h5{font-size:0.9375rem;line-height:1.4rem;font-weight:700;margin-top:0.75rem;margin-bottom:0.3rem;color:#0f172a}',
                'h6{font-size:0.875rem;line-height:1.35rem;font-weight:700;margin-top:0.625rem;margin-bottom:0.25rem;color:#0f172a}',
                'p{margin-bottom:0.75rem;line-height:1.65;color:#1f2937}',
                'strong{font-weight:700}',
                'em{font-style:italic}',
                'u{text-decoration:underline}',
                'ul,ol{padding-left:1.75rem;margin:0.25rem 0;color:#1f2937}',
                'ul{list-style-type:disc}ul ul{list-style-type:circle}ul ul ul{list-style-type:square}',
                'ol{list-style-type:decimal}ol ol{list-style-type:lower-alpha}ol ol ol{list-style-type:lower-roman}',
                'li{margin-bottom:0.15rem}',
                'img{max-width:100%;height:auto;display:block;margin:0.5rem 0}',
                'table{border-collapse:collapse;table-layout:fixed;width:100%;margin:1.5rem 0;border:1px solid #d1d5db}',
                'th,td{border:1px solid #d1d5db;padding:6px 8px;vertical-align:top;box-sizing:border-box;color:#111827}',
                'th{font-weight:700;text-align:left;background-color:#f3f4f6}',
                'tr:nth-child(even) td{background:#fafafa}',
                'pre{background:#f3f4f6;border-radius:6px;padding:0.75rem 1rem;font-family:"Courier New",monospace;font-size:0.875rem;overflow-x:auto;margin:0.75rem 0;color:#111827}',
                'code{font-family:"Courier New",monospace;font-size:0.9em;background:#f3f4f6;padding:0.125rem 0.25rem;border-radius:3px;color:#111827}',
                'pre code{background:transparent;padding:0}',
                'mark{background-color:#fef3c7;color:#92400e;padding:0.1em 0.2em;border-radius:0.2em}',
                'a{color:#2563eb;text-decoration:underline}',
                'blockquote{border-left:3px solid #d1d5db;margin:0.5rem 0;padding:0.25rem 0 0.25rem 1rem;color:#6b7280;font-style:italic}',
            ].join('\n');
            return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>${minimalCSS}</style></head><body>${innerHtml}</body></html>`;
        };

        const filterMap: Record<string, { name: string; extensions: string[] }[]> = {
            txt:  [{ name: '純文字', extensions: ['txt'] }],
            md:   [{ name: 'Markdown', extensions: ['md'] }],
            html: [{ name: 'HTML 文件', extensions: ['html'] }],
            docx: [{ name: 'Word 文件', extensions: ['docx'] }],
            pdf:  [{ name: 'PDF 文件', extensions: ['pdf'] }],
        };
        const filePath = await saveDialog({
            defaultPath: `${name}.${format}`,
            filters: filterMap[format] ?? [],
        });
        if (!filePath) return; // 使用者取消儲存

        if (format === 'txt') {
            await tauriCmd.exportDocument(filePath, text);
        } else if (format === 'md') {
            const TurndownService = (await import('turndown')).default;
            const td = new TurndownService({ headingStyle: 'atx', bulletListMarker: '-' });
            const md = td.turndown(html);
            await tauriCmd.exportDocument(filePath, md);
        } else if (format === 'html') {
            await tauriCmd.exportDocument(filePath, buildStandaloneHtml(html));
        } else if (format === 'docx') {
            const { asBlob } = await import('html-docx-js-typescript');
            const docHtml = buildStandaloneHtml(html);
            const blobOrBuffer = await asBlob(docHtml);
            const bytes = blobOrBuffer instanceof Blob
                ? new Uint8Array(await blobOrBuffer.arrayBuffer())
                : new Uint8Array(blobOrBuffer as unknown as ArrayBufferLike);

            // Convert to base64 in chunks to avoid call stack overflow on large documents.
            let binary = '';
            const CHUNK = 0x8000;
            for (let i = 0; i < bytes.length; i += CHUNK) {
                binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
            }
            await tauriCmd.writeBinaryFile(filePath, btoa(binary));
        } else if (format === 'pdf') {
            const { default: jsPDF } = await import('jspdf');
            const { default: html2canvas } = await import('html2canvas');
            const editorEl = document.querySelector('.ProseMirror') as HTMLElement;
            if (!editorEl) return;
            const canvas = await html2canvas(editorEl, { scale: 2, useCORS: true, backgroundColor: '#ffffff' });
            const imgData = canvas.toDataURL('image/png');
            const pdf = new jsPDF({ orientation: 'p', unit: 'mm', format: 'a4' });
            const pageW = pdf.internal.pageSize.getWidth();
            const pageH = pdf.internal.pageSize.getHeight();
            const imgH = (canvas.height * pageW) / canvas.width;
            let yOffset = 0;
            while (yOffset < imgH) {
                if (yOffset > 0) pdf.addPage();
                pdf.addImage(imgData, 'PNG', 0, -yOffset, pageW, imgH);
                yOffset += pageH;
            }
            pdf.save(filePath);
        }
    };

    return (
        <div className="flex items-center gap-1 border-b border-stroke-divider px-2 py-2 flex-wrap bg-surface-base sticky top-0 z-10" onClick={(e) => e.stopPropagation()}>
            <button
                type="button"
                onClick={(e) => {
                    e.stopPropagation();
                    onOpenDocument();
                }}
                className="flex items-center gap-1 px-2 py-1.5 rounded text-fs-xs font-medium transition-colors text-text-secondary hover:bg-surface-subtle hover:text-text-primary"
                title="開啟文件"
            >
                <FolderOpen className="w-3.5 h-3.5" />
                <span>開啟</span>
            </button>
            <div className="w-px h-5 bg-stroke-divider mx-1" />

            {/* Export Dropdown */}
            <div className="relative" ref={exportMenuRef}>
                <button
                    type="button"
                    onClick={(e) => { e.stopPropagation(); setShowExportMenu(v => !v); }}
                    className={`flex items-center gap-1 px-2 py-1.5 rounded text-fs-xs font-medium transition-colors ${showExportMenu ? 'bg-surface-subtle text-text-primary' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                    title={t('editor.export')}
                >
                    <Upload className="w-3.5 h-3.5" />
                    <span>{t('editor.export')}</span>
                    <ChevronDown className={`w-3 h-3 transition-transform duration-200 ${showExportMenu ? 'rotate-180' : ''}`} />
                </button>
                {showExportMenu && (
                    <div className="absolute left-0 top-full mt-1 w-40 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1.5 px-1.5 ring-1 ring-black/5 animate-in fade-in zoom-in duration-150">
                        {(['txt', 'md', 'html', 'docx', 'pdf'] as const).map(fmt => {
                            const label = fmt === 'html' ? 'HTML' : t(`editor.export_${fmt}` as any);
                            return (
                                <button key={fmt} onClick={() => exportAs(fmt)}
                                    className="w-full px-3 py-1.5 text-fs-xs text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left font-medium">
                                    {label}
                                </button>
                            );
                        })}
                    </div>
                )}
            </div>
            <div className="w-px h-5 bg-stroke-divider mx-1" />

            {/* Turn Into Dropdown ??MenuBar */}
            <div className="relative" ref={turnIntoMenuRef}>
                {(() => {
                    const currentLabel = editor.isActive('heading', { level: 1 }) ? t('turn_into.heading1')
                        : editor.isActive('heading', { level: 2 }) ? t('turn_into.heading2')
                        : editor.isActive('heading', { level: 3 }) ? t('turn_into.heading3')
                        : editor.isActive('heading', { level: 4 }) ? t('turn_into.heading4')
                        : editor.isActive('heading', { level: 5 }) ? t('turn_into.heading5')
                        : editor.isActive('heading', { level: 6 }) ? t('turn_into.heading6')
                        : t('turn_into.text');
                    return (
                <button
                    type="button"
                    onClick={(e) => { e.stopPropagation(); setShowTurnIntoMenu(v => !v); }}
                    className={`flex items-center gap-1 px-2 py-1.5 rounded text-fs-xs font-medium transition-colors ${showTurnIntoMenu ? 'bg-surface-subtle text-text-primary' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                    title={t('turn_into.label')}
                >
                    <span>{currentLabel}</span>
                    <ChevronDown className={`w-3 h-3 transition-transform duration-200 ${showTurnIntoMenu ? 'rotate-180' : ''}`} />
                </button>
                    );
                })()}
                {showTurnIntoMenu && (
                    <div className="absolute left-0 top-full mt-1 w-44 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1 px-1 ring-1 ring-black/5 animate-in fade-in zoom-in duration-150">
                        {([
                            { key: 'text', label: t('turn_into.text'), icon: 'T', action: () => editor.chain().focus().setParagraph().run(), isActive: editor.isActive('paragraph') && !editor.isActive('heading') },
                            { key: 'h1', label: t('turn_into.heading1'), icon: 'H1', action: () => editor.chain().focus().setHeading({ level: 1 }).run(), isActive: editor.isActive('heading', { level: 1 }) },
                            { key: 'h2', label: t('turn_into.heading2'), icon: 'H2', action: () => editor.chain().focus().setHeading({ level: 2 }).run(), isActive: editor.isActive('heading', { level: 2 }) },
                            { key: 'h3', label: t('turn_into.heading3'), icon: 'H3', action: () => editor.chain().focus().setHeading({ level: 3 }).run(), isActive: editor.isActive('heading', { level: 3 }) },
                            { key: 'h4', label: t('turn_into.heading4'), icon: 'H4', action: () => editor.chain().focus().setHeading({ level: 4 }).run(), isActive: editor.isActive('heading', { level: 4 }) },
                            { key: 'h5', label: t('turn_into.heading5'), icon: 'H5', action: () => editor.chain().focus().setHeading({ level: 5 }).run(), isActive: editor.isActive('heading', { level: 5 }) },
                            { key: 'h6', label: t('turn_into.heading6'), icon: 'H6', action: () => editor.chain().focus().setHeading({ level: 6 }).run(), isActive: editor.isActive('heading', { level: 6 }) },
                        ] as const).map(item => (
                            <button
                                key={item.key}
                                onClick={() => { item.action(); setShowTurnIntoMenu(false); }}
                                className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium rounded-md transition-colors text-left ${item.isActive ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                            >
                                <span className="w-5 text-center text-[10px] font-bold shrink-0 text-text-tertiary">{item.icon}</span>
                                {item.label}
                            </button>
                        ))}
                    </div>
                )}
            </div>

            <div className="w-px h-5 bg-stroke-divider mx-1" />
            <ToolbarButton
                icon={Bold}
                title="粗體 (Ctrl+B)"
                onClick={() => editor.chain().focus().toggleBold().run()}
                isActive={editor.isActive('bold')}
            />
            <ToolbarButton
                icon={Italic}
                title="斜體 (Ctrl+I)"
                onClick={() => editor.chain().focus().toggleItalic().run()}
                isActive={editor.isActive('italic')}
            />
            <ToolbarButton
                icon={UnderlineIcon}
                title="底線 (Ctrl+U)"
                onClick={() => editor.chain().focus().toggleUnderline().run()}
                isActive={editor.isActive('underline')}
            />
            <ToolbarButton
                icon={Highlighter}
                title="螢光筆"
                onClick={() => editor.chain().focus().toggleHighlight().run()}
                isActive={editor.isActive('highlight')}
            />
            <div className="w-px h-5 bg-stroke-divider mx-1" />
            <ToolbarButton
                icon={Heading1}
                title="標題 1 (Ctrl+Alt+1)"
                onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
                isActive={editor.isActive('heading', { level: 1 })}
            />
            <ToolbarButton
                icon={Heading2}
                title="標題 2 (Ctrl+Alt+2)"
                onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
                isActive={editor.isActive('heading', { level: 2 })}
            />
            <div className="w-px h-5 bg-stroke-divider mx-1" />

            {/* List Dropdown ??MenuBar */}
            <div className="relative" ref={listMenuRef}>
                <button
                    type="button"
                    onClick={(e) => { e.stopPropagation(); setShowListMenu(v => !v); }}
                    className={`flex items-center gap-1 p-1.5 rounded transition-colors ${editor.isActive('bulletList') || editor.isActive('orderedList') ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                    title={t('list_dropdown.label')}
                >
                    {editor.isActive('orderedList') ? <ListOrdered className="w-4 h-4" /> : <List className="w-4 h-4" />}
                    <ChevronDown className={`w-3 h-3 transition-transform duration-200 ${showListMenu ? 'rotate-180' : ''}`} />
                </button>
                {showListMenu && (
                    <div className="absolute left-0 top-full mt-1 w-44 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1 px-1 ring-1 ring-black/5 animate-in fade-in zoom-in duration-150">
                        <button
                            onClick={() => { editor.chain().focus().toggleBulletList().run(); setShowListMenu(false); }}
                            className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium rounded-md transition-colors text-left ${editor.isActive('bulletList') ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                        >
                            <List className="w-3.5 h-3.5 shrink-0" />
                            {t('list_dropdown.bullet_list')}
                        </button>
                        <button
                            onClick={() => { editor.chain().focus().toggleOrderedList().run(); setShowListMenu(false); }}
                            className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium rounded-md transition-colors text-left ${editor.isActive('orderedList') ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                        >
                            <ListOrdered className="w-3.5 h-3.5 shrink-0" />
                            {t('list_dropdown.ordered_list')}
                        </button>
                    </div>
                )}
            </div>

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            {/* Table Controls with Dropdown */}
            <div className="relative" ref={tableMenuRef}>
                <button
                    type="button"
                    onClick={(e) => {
                        e.stopPropagation();
                        if (editor.isActive('table')) {
                            setShowTableMenu(!showTableMenu);
                        } else {
                            editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
                        }
                    }}
                    title={editor.isActive('table') ? "表格操作" : "插入表格"}
                    className={`p-1.5 rounded transition-all flex items-center gap-0.5 ${editor.isActive('table')
                        ? 'bg-accent-light2 text-accent-default shadow-sm'
                        : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'
                        }`}
                >
                    <TableIcon className="w-4 h-4" />
                    <ChevronDown className={`w-3 h-3 transition-transform duration-200 ${showTableMenu ? 'rotate-180' : ''}`} />
                </button>

                {showTableMenu && (
                    <div className="absolute left-0 top-full mt-1 w-56 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1.5 px-1.5 overflow-hidden ring-1 ring-black/5 animate-in fade-in zoom-in duration-150">
                        <div className="px-3 py-1.5 text-[10px] font-bold text-text-tertiary uppercase tracking-wider mb-1">
                            行列管理
                        </div>
                        <MenuAction
                            icon={Plus}
                            label="於上方新增列"
                            onClick={() => editor.chain().focus().addRowBefore().run()}
                        />
                        <MenuAction
                            icon={Plus}
                            label="於下方新增列"
                            onClick={() => editor.chain().focus().addRowAfter().run()}
                        />
                        <MenuAction
                            icon={Columns}
                            label="於左方新增欄"
                            onClick={() => editor.chain().focus().addColumnBefore().run()}
                        />
                        <MenuAction
                            icon={Columns}
                            label="於右方新增欄"
                            onClick={() => editor.chain().focus().addColumnAfter().run()}
                        />
                        <div className="h-px bg-stroke-divider my-1.5 mx-1" />
                        <div className="px-3 py-1.5 text-[10px] font-bold text-text-tertiary uppercase tracking-wider mb-1">
                            表格操作
                        </div>
                        <MenuAction
                            icon={Merge}
                            label="合併格"
                            onClick={() => editor.chain().focus().mergeCells().run()}
                            disabled={!editor.can().mergeCells()}
                        />
                        <MenuAction
                            icon={Split}
                            label="分割格"
                            onClick={() => editor.chain().focus().splitCell().run()}
                            disabled={!editor.can().splitCell()}
                        />
                        <MenuAction
                            icon={LayoutTemplate}
                            label="切換首行標題"
                            onClick={() => editor.chain().focus().toggleHeaderRow().run()}
                        />
                        <div className="h-px bg-stroke-divider my-1.5 mx-1" />
                        <MenuAction
                            icon={Trash2}
                                                        label="刪除此行"
                            onClick={() => editor.chain().focus().deleteRow().run()}
                            danger
                        />
                        <MenuAction
                            icon={Trash2}
                                                        label="刪除此列"
                            onClick={() => editor.chain().focus().deleteColumn().run()}
                            danger
                        />
                        <MenuAction
                            icon={Trash2}
                                                        label="刪除整個表格"
                            onClick={() => editor.chain().focus().deleteTable().run()}
                            danger
                        />
                    </div>
                )}
            </div>

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            {/* Image Controls */}
            <label className="p-1.5 rounded transition-colors text-text-secondary hover:bg-surface-subtle cursor-pointer" title="上傳圖片">
                <ImageIcon className="w-4 h-4" />
                <input
                    type="file"
                    className="hidden"
                    accept="image/*"
                    onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (file) {
                            const reader = new FileReader();
                            reader.onload = (event) => {
                                const src = event.target?.result as string;
                                if (editor.commands.setImage) {
                                    editor.chain().focus().setImage({ src }).run();
                                } else {
                                    editor.chain().focus().insertContent({
                                        type: 'imageNodePro',
                                        attrs: { src }
                                    }).run();
                                }
                            };
                            reader.readAsDataURL(file);
                        }
                    }}
                />
            </label>

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <ToolbarButton
                icon={Code}
                title="程式碼區塊"
                onClick={() => editor.chain().focus().toggleCodeBlock().run()}
                isActive={editor.isActive('codeBlock')}
            />
        </div>
    );
};

function calcPanelStyle(anchor: { x: number; y: number }): React.CSSProperties {
    const PANEL_WIDTH = 320;
    const GAP = 12;
    const ESTIMATED_H = 180;
    const vw = window.innerWidth;
    let left = Math.min(anchor.x, vw - PANEL_WIDTH - 16);
    left = Math.max(left, 8);
    const showAbove = anchor.y - ESTIMATED_H - GAP > 0;
    return showAbove
        ? { position: 'fixed', left: `${left}px`, top: `${anchor.y - GAP}px`, transform: 'translateY(-100%)', width: `${PANEL_WIDTH}px`, zIndex: 200 }
        : { position: 'fixed', left: `${left}px`, top: `${anchor.y + GAP + 20}px`, width: `${PANEL_WIDTH}px`, zIndex: 200 };
}

interface EditorTab {
    id: string;
    title: string;
    content: string;
    filePath?: string;
}

function escapeHtml(value: string): string {
    return value
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function plainTextToHtml(value: string): string {
    const normalized = value.replace(/\r\n/g, '\n').trim();
    if (!normalized) return '<p></p>';

    return normalized
        .split(/\n{2,}/)
        .map(block => `<p>${escapeHtml(block).replace(/\n/g, '<br />')}</p>`)
        .join('');
}

let tabCounter = (() => {
    try {
        const raw = localStorage.getItem('notes:session');
        const s = raw ? JSON.parse(raw) : null;
        return (s?.tabCounter as number) ?? 1;
    } catch { return 1; }
})();

export const EditorPane: React.FC = () => {
    useUiStore();
    const t = useT();

    const [tabs, setTabs] = useState<EditorTab[]>(() => {
        const s = loadSession();
        if (s?.tabs?.length) return s.tabs.map(t => ({ id: t.id, title: t.title, content: '' }));
        return [{ id: 'tab-1', title: '新文件 1', content: '' }];
    });
    const [activeTabId, setActiveTabId] = useState<string>(() => loadSession()?.activeTabId ?? 'tab-1');
    const tabContentsRef = useRef<Record<string, string>>((() => {
        const s = loadSession();
        const tabList = s?.tabs ?? [{ id: 'tab-1' }];
        const result: Record<string, string> = {};
        for (const t of tabList) result[t.id] = loadContent(t.id);
        return result;
    })());
    // History panel state
    const [showHistoryMenu, setShowHistoryMenu] = useState(false);
    const [historyEntries, setHistoryEntries] = useState<HistoryEntry[]>([]);
    const historyMenuRef = useRef<HTMLDivElement>(null);
    const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const TabHandler = Extension.create({
        name: 'tabHandler',
        addKeyboardShortcuts() {
            return {
                Tab: () => {
                    if (this.editor.isActive('listItem')) {
                        // 清單項目中以 Tab 增加縮排層級。
                        this.editor.commands.sinkListItem('listItem');
                        return true;
                    }
                    // 非清單狀態時插入兩個空白。
                    this.editor.commands.insertContent('  ');
                    return true;
                },
                'Shift-Tab': () => {
                    if (this.editor.isActive('listItem')) {
                        // 清單項目中以 Shift+Tab 減少縮排層級。
                        this.editor.commands.liftListItem('listItem');
                        return true;
                    }
                    return false;
                },
            };
        },
    });

    const editor = useEditor({
        extensions: [
            StarterKit,
            Table.configure({
                resizable: true,
                allowTableNodeSelection: false,
                lastColumnResizable: true,
                handleWidth: 10,
            }),
            TableRow,
            TableHeader,
            TableCell,
            BubbleMenuExtension,
            ImageNodePro.configure({
                HTMLAttributes: {
                    class: 'rounded-lg max-w-full',
                },
            }),
            Placeholder.configure({
                placeholder: '在這裡撰寫筆記、報告或草稿...',
            }),
            Highlight,
            Underline,
            TextAlign.configure({
                types: ['heading', 'paragraph', 'imageNodePro'],
            }),
            Link.configure({
                openOnClick: false,
                HTMLAttributes: {
                    class: 'text-fs-sm xl:text-fs-base text-text-link hover:text-text-link-hover underline cursor-pointer',
                },
            }),
            TabHandler,
        ],
        content: '',
        coreExtensionOptions: {
            clipboardTextSerializer: {
                blockSeparator: '\n',
            },
        },
        editorProps: {
            attributes: {
                class: 'focus:outline-none min-h-[500px] max-w-none px-12 pt-14 pb-6 text-fs-sm xl:text-fs-base',
            },
        },
    });

    // 切換 Tab 時，先保存目前內容，再載入新分頁內容。
    const switchTab = useCallback((newTabId: string) => {
        if (!editor) return;
        // 保存目前 tab 並記錄快照
        const json = JSON.stringify(editor.getJSON());
        tabContentsRef.current[activeTabId] = json;
        saveContent(activeTabId, json);
        addHistory(activeTabId, json);
        // 載入新 tab 內容
        const saved = tabContentsRef.current[newTabId];
        if (saved) {
            try { editor.commands.setContent(JSON.parse(saved)); } catch { editor.commands.setContent(''); }
        } else {
            editor.commands.setContent('');
        }
        setActiveTabId(newTabId);
    }, [editor, activeTabId]);

    const addTab = useCallback(() => {
        if (!editor) return;
        const curJson = JSON.stringify(editor.getJSON());
        tabContentsRef.current[activeTabId] = curJson;
        saveContent(activeTabId, curJson);
        tabCounter += 1;
        const newId = `tab-${Date.now()}`;
        const newTitle = `新文件 ${tabCounter}`;
        const now = Date.now();
        upsertFile({ id: newId, title: newTitle, createdAt: now, updatedAt: now });
        tabContentsRef.current[newId] = '';
        setTabs(prev => [...prev, { id: newId, title: newTitle, content: '' }]);
        editor.commands.setContent('');
        setActiveTabId(newId);
    }, [editor, activeTabId]);

    const closeTab = useCallback((tabId: string, e: React.MouseEvent) => {
        e.stopPropagation();
        // Save content to localStorage before closing (keep it for future reopen)
        if (editor) {
            const closingJson = tabId === activeTabId
                ? JSON.stringify(editor.getJSON())
                : (tabContentsRef.current[tabId] ?? '');
            if (closingJson) saveContent(tabId, closingJson);
        }
        setTabs(prev => {
            if (prev.length <= 1) return prev; // 至少保留一個 tab
            const idx = prev.findIndex(t => t.id === tabId);
            const next = prev.filter(t => t.id !== tabId);
            delete tabContentsRef.current[tabId]; // 從記憶體移除，localStorage 保留
            // 切換到鄰近 tab
            if (tabId === activeTabId && editor) {
                const nextTab = next[Math.min(idx, next.length - 1)];
                const saved = tabContentsRef.current[nextTab.id] ?? loadContent(nextTab.id);
                tabContentsRef.current[nextTab.id] = saved;
                if (saved) {
                    try { editor.commands.setContent(JSON.parse(saved)); } catch { editor.commands.setContent(''); }
                } else {
                    editor.commands.setContent('');
                }
                setActiveTabId(nextTab.id);
            }
            return next;
        });
    }, [activeTabId, editor]);

    const [renamingTabId, setRenamingTabId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState('');

    const commitRename = useCallback((tabId: string) => {
        const trimmed = renameValue.trim();
        if (trimmed) {
            setTabs(prev => prev.map(t => t.id === tabId ? { ...t, title: trimmed } : t));
            renameNoteFile(tabId, trimmed);
        }
        setRenamingTabId(null);
    }, [renameValue]);

    // ── localStorage persistence effects ──────────────────────────────────────

    // Load persisted content into editor when it first becomes ready
    useEffect(() => {
        if (!editor) return;
        const saved = tabContentsRef.current[activeTabId];
        if (saved) {
            try { editor.commands.setContent(JSON.parse(saved)); } catch { editor.commands.setContent(''); }
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [editor]); // run once when editor initialises

    // Auto-save on every edit (debounced 1.5 s)
    useEffect(() => {
        if (!editor) return;
        const onUpdate = () => {
            if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
            autoSaveTimerRef.current = setTimeout(() => {
                const json = JSON.stringify(editor.getJSON());
                tabContentsRef.current[activeTabId] = json;
                saveContent(activeTabId, json);
            }, 1500);
        };
        editor.on('update', onUpdate);
        return () => {
            editor.off('update', onUpdate);
            if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
        };
    }, [editor, activeTabId]);

    // Persist session (open tabs + active tab + counter) on every change
    useEffect(() => {
        saveSession({ activeTabId, tabs: tabs.map(t => ({ id: t.id, title: t.title })), tabCounter });
    }, [tabs, activeTabId]);

    // Periodic history snapshot every 5 minutes
    useEffect(() => {
        if (!editor) return;
        const interval = setInterval(() => {
            addHistory(activeTabId, JSON.stringify(editor.getJSON()));
        }, 5 * 60 * 1000);
        return () => clearInterval(interval);
    }, [editor, activeTabId]);

    // Register initial tabs in file registry (handles first-boot)
    useEffect(() => {
        const now = Date.now();
        const existingIds = new Set(loadFiles().map(f => f.id));
        const toAdd = tabs.filter(t => !existingIds.has(t.id));
        if (toAdd.length > 0) {
            toAdd.forEach(t => upsertFile({ id: t.id, title: t.title, createdAt: now, updatedAt: now }));
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []); // only on mount

    // ── File helpers ───────────────────────────────────────────────────────────

    const formatTs = (ts: number) => {
        const d = new Date(ts);
        return `${d.getMonth() + 1}/${d.getDate()} ${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
    };

    const openDocumentFromPath = useCallback((path: string, content: string) => {
        if (!editor) return;
        const existingTab = tabs.find(t => t.filePath === path);
        if (existingTab) {
            tabContentsRef.current[activeTabId] = JSON.stringify(editor.getJSON());
            saveContent(activeTabId, tabContentsRef.current[activeTabId]);
            setTabs(prev => prev.map(tab => tab.id === existingTab.id ? { ...tab, title: path.split(/[\\/]/).pop() || tab.title } : tab));
            editor.commands.setContent(plainTextToHtml(content));
            const json = JSON.stringify(editor.getJSON());
            tabContentsRef.current[existingTab.id] = json;
            saveContent(existingTab.id, json);
            setActiveTabId(existingTab.id);
            return;
        }

        const curJson = JSON.stringify(editor.getJSON());
        tabContentsRef.current[activeTabId] = curJson;
        saveContent(activeTabId, curJson);

        const newId = `tab-${Date.now()}`;
        const title = path.split(/[\\/]/).pop() || '文件';
        setTabs(prev => [...prev, { id: newId, title, content: '', filePath: path }]);
        editor.commands.setContent(plainTextToHtml(content));
        const json = JSON.stringify(editor.getJSON());
        tabContentsRef.current[newId] = json;
        saveContent(newId, json);
        setActiveTabId(newId);
    }, [editor, activeTabId, tabs]);

    const handleOpenDocument = useCallback(async () => {
        if (!editor) return;

        const selected = await openDialog({
            multiple: false,
            directory: false,
            filters: [
                { name: '文件', extensions: ['txt', 'md', 'markdown', 'html', 'htm'] },
            ],
        });

        if (!selected || Array.isArray(selected)) return;

        try {
            const content = await tauriCmd.openDocument(selected);
            openDocumentFromPath(selected, content);
        } catch (error) {
            console.error('Open document failed:', error);
        }
    }, [editor, openDocumentFromPath]);

    // Restore a history snapshot for the active tab
    const restoreHistory = useCallback((entry: HistoryEntry) => {
        if (!editor) return;
        addHistory(activeTabId, JSON.stringify(editor.getJSON()));
        try {
            editor.commands.setContent(JSON.parse(entry.snapshot));
            tabContentsRef.current[activeTabId] = entry.snapshot;
            saveContent(activeTabId, entry.snapshot);
        } catch {}
        setShowHistoryMenu(false);
    }, [editor, activeTabId]);

    const [isAiImproving, setIsAiImproving] = useState(false);
    const [aiImproveResult, setAiImproveResult] = useState<string | null>(null);
    const [aiPanelAnchor, setAiPanelAnchor] = useState<{ x: number; y: number } | null>(null);
    const [showAiDropdown, setShowAiDropdown] = useState(false);
    const [aiDropdownType, setAiDropdownType] = useState<'main' | 'tone' | 'translate' | 'expand' | 'shorten' | 'custom'>('main');
    const [customPrompt, setCustomPrompt] = useState('');
    const aiDropdownRef = useRef<HTMLDivElement>(null);
    const aiPanelRef = useRef<HTMLDivElement>(null);
    const [showBubbleTurnInto, setShowBubbleTurnInto] = useState(false);
    const bubbleTurnIntoRef = useRef<HTMLDivElement>(null);

    const lastPromptRef = useRef<string>('');
    const selectedTextRef = useRef<string>('');

        // 表格工具選單位置（fixed，固定在表格上方置中）
    const [tableMenuPos, setTableMenuPos] = useState<{ x: number; y: number } | null>(null);

    useEffect(() => {
        if (!editor) return;
        const update = () => {
            if (editor.isActive('table')) {
                // 找到目前選取所在的 table DOM 節點
                const { state, view } = editor;
                const { $from } = state.selection;
                let tablePos: number | null = null;
                for (let d = $from.depth; d >= 0; d--) {
                    if ($from.node(d).type.name === 'table') {
                        tablePos = $from.before(d);
                        break;
                    }
                }
                if (tablePos !== null) {
                    const domNode = view.nodeDOM(tablePos) as HTMLElement | null;
                    const tableEl = domNode?.nodeName === 'TABLE' ? domNode : domNode?.querySelector('table') ?? domNode;
                    if (tableEl) {
                        const rect = tableEl.getBoundingClientRect();
                        setTableMenuPos({ x: rect.left + rect.width / 2, y: rect.top });
                    }
                }
            } else {
                setTableMenuPos(null);
            }
        };
        editor.on('selectionUpdate', update);
        editor.on('transaction', update);
        return () => {
            editor.off('selectionUpdate', update);
            editor.off('transaction', update);
        };
    }, [editor]);

    // 文字彈出選單位置（fixed，mouseup 後延遲顯示，避免拖選時跳動）
    const [textBubblePos, setTextBubblePos] = useState<{ x: number; y: number } | null>(null);
    const textBubbleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
        if (!editor) return;

        const showBubble = () => {
            if (textBubbleTimer.current) { clearTimeout(textBubbleTimer.current); textBubbleTimer.current = null; }
            textBubbleTimer.current = setTimeout(() => {
                if (!editor || isAiImproving || aiImproveResult) return;
                if ((editor.state.selection as any).$anchorCell) return;
                const { from, to } = editor.state.selection;
                if (from === to || editor.isActive('imageNodePro')) return;
                const { view } = editor;
                const start = view.coordsAtPos(from);
                const end = view.coordsAtPos(to);
                setTextBubblePos({ x: (start.left + end.right) / 2, y: Math.min(start.top, end.top) });
            }, 250);
        };

        const hideBubble = () => {
            if (textBubbleTimer.current) { clearTimeout(textBubbleTimer.current); textBubbleTimer.current = null; }
            const { from, to } = editor.state.selection;
            if (from === to) setTextBubblePos(null);
        };

        // mouseup 時延遲計算位置
        const editorDom = editor.view.dom;
        editorDom.addEventListener('mouseup', showBubble);
        // 鍵盤選取也要支援
        editorDom.addEventListener('keyup', showBubble);
        // 選取消失時立即隱藏
        editor.on('selectionUpdate', hideBubble);

        return () => {
            editorDom.removeEventListener('mouseup', showBubble);
            editorDom.removeEventListener('keyup', showBubble);
            editor.off('selectionUpdate', hideBubble);
            if (textBubbleTimer.current) clearTimeout(textBubbleTimer.current);
        };
    }, [editor, isAiImproving, aiImproveResult]);

    // AI 改寫邏輯（Streaming 版本）
    const handleAiImprove = useCallback(async (prompt: string) => {
        if (!editor || isAiImproving) return;

        const { from, to } = editor.state.selection;
        const text = editor.state.doc.textBetween(from, to);
        if (!text.trim()) return;

        lastPromptRef.current = prompt;
        selectedTextRef.current = text;

        // 記錄選取起點的 viewport 座標，供 AiResultPanel 定位
        const coords = editor.view.coordsAtPos(from);
        setAiPanelAnchor(prev => prev ?? { x: coords.left, y: coords.top });

        setIsAiImproving(true);
        setAiImproveResult('');
        setShowAiDropdown(false);

        const convId = `ai-improve-${Date.now()}`;
        const fullPrompt = `${prompt}\n\n原文內容：\n${text}`;

        try {
            const unlisteners: Array<() => void> = [];

            await new Promise<void>((resolve, reject) => {
                Promise.all([
                    listen<{ conversationId: string; token: string }>('rag-stream-token', (event) => {
                        if (event.payload.conversationId !== convId) return;
                        setAiImproveResult(prev => (prev ?? '') + event.payload.token);
                    }),
                    listen<{ conversationId: string; fullAnswer: string }>('rag-stream-done', (event) => {
                        if (event.payload.conversationId !== convId) return;
                        setAiImproveResult(event.payload.fullAnswer);
                        resolve();
                    }),
                ]).then(([ut, ud]) => {
                    unlisteners.push(ut, ud);
                });

                invoke('rag_query_stream', {
                    query: fullPrompt,
                    conversationId: convId,
                    history: [],
                    conversationSummary: null,
                    projectId: null,
                    sourceIds: null,
                    tagFilter: null,
                    ragEnabled: false,
                    webEnabled: false,
                    tempChunkIds: null,
                    thinkingMode: 'normal',
                }).catch(reject);
            });

            unlisteners.forEach(fn => fn());
        } catch (error) {
            console.error('AI Improve failed:', error);
            setAiImproveResult(null);
        } finally {
            setIsAiImproving(false);
        }
    }, [editor, isAiImproving]);

    // 點擊外部時關閉下拉選單 / AI 結果面板
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (aiDropdownRef.current && !aiDropdownRef.current.contains(event.target as Node)) {
                setShowAiDropdown(false);
                setAiDropdownType('main');
            }
            if (aiPanelRef.current && !aiPanelRef.current.contains(event.target as Node)) {
                if (aiImproveResult && !isAiImproving) {
                    setAiImproveResult(null);
                    setAiPanelAnchor(null);
                }
            }
            if (bubbleTurnIntoRef.current && !bubbleTurnIntoRef.current.contains(event.target as Node)) {
                setShowBubbleTurnInto(false);
            }
            if (historyMenuRef.current && !historyMenuRef.current.contains(event.target as Node)) {
                setShowHistoryMenu(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [aiImproveResult, isAiImproving]);

    return (
        <div className="w-full h-full flex flex-col border-l border-stroke-divider bg-surface-base transition-all duration-300" onClick={() => editor?.commands.focus()}>
            {/* Tab Bar */}
            <div className="flex items-center border-b border-stroke-divider bg-surface-layer shrink-0 h-12" onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center min-w-0 flex-1 overflow-x-auto">
                    {tabs.map(tab => (
                        <div
                            key={tab.id}
                            onClick={() => tab.id !== activeTabId && switchTab(tab.id)}
                            onDoubleClick={() => { setRenamingTabId(tab.id); setRenameValue(tab.title); }}
                            className={`group flex items-center gap-1.5 px-3 py-2 text-fs-xs font-medium cursor-pointer shrink-0 border-r border-stroke-divider transition-colors select-none ${
                                tab.id === activeTabId
                                    ? 'bg-surface-base text-text-primary border-b-2 border-b-accent-default -mb-px'
                                    : 'text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle'
                            }`}
                        >
                            {renamingTabId === tab.id ? (
                                <input
                                    autoFocus
                                    value={renameValue}
                                    onChange={e => setRenameValue(e.target.value)}
                                    onBlur={() => commitRename(tab.id)}
                                    onKeyDown={e => {
                                        if (e.key === 'Enter') { e.preventDefault(); commitRename(tab.id); }
                                        if (e.key === 'Escape') setRenamingTabId(null);
                                        e.stopPropagation();
                                    }}
                                    onClick={e => e.stopPropagation()}
                                    className="max-w-[100px] bg-surface-base border border-accent-default rounded px-1 text-fs-xs text-text-primary outline-none"
                                />
                            ) : (
                                <span className="max-w-[100px] truncate">{tab.title}</span>
                            )}
                            {tabs.length > 1 && renamingTabId !== tab.id && (
                                <button
                                    onClick={(e) => closeTab(tab.id, e)}
                                    className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-elevated transition-all text-text-tertiary"
                                >
                                    <X className="w-3 h-3" />
                                </button>
                            )}
                        </div>
                    ))}
                    <button
                        onClick={addTab}
                        className="px-2 py-1.5 ml-1 text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle rounded transition-colors shrink-0 text-fs-sm leading-none"
                        title="新增文件"
                    >
                        +
                    </button>
                </div>
                </div>

                {/* Menu Bar */}
                <MenuBar
                    editor={editor}
                    fileName={tabs.find(t => t.id === activeTabId)?.title ?? '文件'}
                    onOpenDocument={handleOpenDocument}
                />

            {/* Image Bubble Menu，僅在選中圖片且不在表格內時顯示 */}
            {editor && (
                <BubbleMenu
                    editor={editor}
                    shouldShow={({ editor }) => editor.isActive('imageNodePro') && !editor.isActive('table')}
                    options={{
                        offset: 8,
                        placement: 'top',
                    }}
                >
                    <div className="flex items-center gap-0.5 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl px-2 py-1.5 animate-in fade-in zoom-in duration-200 z-[100] mx-6">
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { textAlign: 'left' }).run()}
                            className={`p-1.5 rounded hover:bg-surface-subtle transition-colors ${editor.isActive('imageNodePro', { textAlign: 'left' }) ? 'text-accent-default bg-accent-light2' : 'text-text-secondary'}`}
                            title="對齊置左"
                        >
                            <AlignLeft className="w-4 h-4" />
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { textAlign: 'center' }).run()}
                            className={`p-1.5 rounded hover:bg-surface-subtle transition-colors ${editor.isActive('imageNodePro', { textAlign: 'center' }) ? 'text-accent-default bg-accent-light2' : 'text-text-secondary'}`}
                            title="對齊置中"
                        >
                            <AlignCenter className="w-4 h-4" />
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { textAlign: 'right' }).run()}
                            className={`p-1.5 rounded hover:bg-surface-subtle transition-colors ${editor.isActive('imageNodePro', { textAlign: 'right' }) ? 'text-accent-default bg-accent-light2' : 'text-text-secondary'}`}
                            title="對齊置右"
                        >
                            <AlignRight className="w-4 h-4" />
                        </button>
                        <div className="w-px h-4 bg-stroke-divider mx-1" />
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { width: '25%' }).run()}
                            className="px-1.5 py-1 text-[10px] font-bold text-text-secondary hover:bg-surface-subtle rounded transition-colors"
                        >
                            25%
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { width: '50%' }).run()}
                            className="px-1.5 py-1 text-[10px] font-bold text-text-secondary hover:bg-surface-subtle rounded transition-colors"
                        >
                            50%
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { width: '100%' }).run()}
                            className="px-1.5 py-1 text-[10px] font-bold text-text-secondary hover:bg-surface-subtle rounded transition-colors"
                        >
                            100%
                        </button>
                        <div className="w-px h-4 bg-stroke-divider mx-1" />
                        <button
                            onClick={() => editor.chain().focus().deleteSelection().run()}
                            className="p-1.5 rounded hover:bg-status-error/10 text-status-error transition-colors"
                            title="刪除圖片"
                        >
                            <Trash2 className="w-4 h-4" />
                        </button>
                    </div>
                </BubbleMenu>
            )}

            {/* Text Bubble Menu */}
            {editor && textBubblePos && (
                <div
                    style={{
                        position: 'fixed',
                        left: `${textBubblePos.x}px`,
                        top: `${textBubblePos.y - 10}px`,
                        transform: 'translate(-50%, -100%)',
                        zIndex: 100,
                    }}
                    onMouseDown={e => e.stopPropagation()}
                >
                    <div className="flex items-center gap-0.5 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl p-1 animate-in fade-in zoom-in duration-200 z-[100] whitespace-nowrap">
                        <div className="flex items-center gap-0.5 flex-nowrap">
                                {/* Improve Dropdown Trigger */}
                                <div className="relative" ref={aiDropdownRef}>
                                    <button
                                        onClick={() => { setShowAiDropdown(v => !v); setAiDropdownType('main'); }}
                                        className={`flex items-center gap-1 px-2 py-1.5 rounded hover:bg-accent-light2 text-accent-default transition-colors font-bold text-fs-xs ${showAiDropdown ? 'bg-accent-light2' : ''}`}
                                        title={t('editor.ai_improve')}
                                    >
                                        <Sparkles className="w-3.5 h-3.5" />
                                        <span>{t('editor.ai_improve')}</span>
                                        <ChevronDown className={`w-3 h-3 transition-transform ${showAiDropdown ? 'rotate-180' : ''}`} />
                                    </button>

                                    {/* Custom Dropdown Menu */}
                                    {showAiDropdown && (
                                        <div
                                            className={`absolute left-0 top-full mt-1 bg-surface-base border border-stroke-divider rounded-lg shadow-xl p-1 z-[110] animate-in fade-in slide-in-from-top-1 duration-200 ${aiDropdownType === 'custom' ? 'w-72' : 'w-52'} ${aiDropdownType === 'tone' ? 'max-h-72 overflow-y-auto' : ''}`}
                                            onMouseDown={e => e.stopPropagation()}
                                        >
                                            {aiDropdownType === 'main' && (
                                                <>
                                                    <button onClick={() => handleAiImprove('請修正以下文字的語法錯誤，使其更流暢自然。直接輸出修正後的文字，不要附上任何解釋：')} className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <Wand2 className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_fix_grammar')}
                                                    </button>
                                                    <button onClick={() => setAiDropdownType('expand')} className="w-full flex items-center justify-between px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <div className="flex items-center gap-2.5">
                                                            <FileText className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_extend')}
                                                        </div>
                                                        <ChevronRight className="w-3 h-3 shrink-0 text-text-tertiary" />
                                                    </button>
                                                    <button onClick={() => setAiDropdownType('shorten')} className="w-full flex items-center justify-between px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <div className="flex items-center gap-2.5">
                                                            <Eraser className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_shorten')}
                                                        </div>
                                                        <ChevronRight className="w-3 h-3 shrink-0 text-text-tertiary" />
                                                    </button>
                                                    <button onClick={() => handleAiImprove('請將以下文字簡化，使其更簡單易懂，並保留原意。直接輸出簡化後的文字，不要附上任何解釋：')} className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <Eraser className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_simplify')}
                                                    </button>
                                                    <div className="h-px bg-stroke-divider my-1 mx-1" />
                                                    <button onClick={() => setAiDropdownType('tone')} className="w-full flex items-center justify-between px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <div className="flex items-center gap-2.5">
                                                            <Smile className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_adjust_tone')}
                                                        </div>
                                                        <ChevronRight className="w-3 h-3 shrink-0 text-text-tertiary" />
                                                    </button>
                                                    <button onClick={() => handleAiImprove('請將以下段落補寫完整，使其內容更完整自然。直接輸出補寫後的文字，不要附上任何解釋：')} className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <RefreshCw className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_complete')}
                                                    </button>
                                                    <button onClick={() => handleAiImprove('請將以下段落整理為簡潔摘要。直接輸出摘要，不要附上任何解釋：')} className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <FileText className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_summarize')}
                                                    </button>
                                                    <button onClick={() => setAiDropdownType('translate')} className="w-full flex items-center justify-between px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <div className="flex items-center gap-2.5">
                                                            <Languages className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_translate')}
                                                        </div>
                                                        <ChevronRight className="w-3 h-3 shrink-0 text-text-tertiary" />
                                                    </button>
                                                    <div className="h-px bg-stroke-divider my-1 mx-1" />
                                                    <button onClick={() => { setAiDropdownType('custom'); setCustomPrompt(''); }} className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                        <Sparkles className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_custom')}
                                                    </button>
                                                </>
                                            )}
                                            {aiDropdownType === 'expand' && (
                                                <>
                                                    <button onClick={() => setAiDropdownType('main')} className="w-full flex items-center gap-2 px-2 py-1 text-fs-xs text-text-tertiary hover:text-text-secondary rounded-md transition-colors mb-0.5">
                                                        <RotateCcw className="w-3 h-3 shrink-0" /> {t('editor.ai_back')}
                                                    </button>
                                                    <div className="px-2.5 py-1 text-[10px] font-medium text-text-tertiary">{t('editor.ai_extend_degree')}</div>
                                                    <button onClick={() => handleAiImprove('請適度擴展以下文字，補充少量細節並保持簡潔。直接輸出擴展後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_extend_slight')}</button>
                                                    <button onClick={() => handleAiImprove('請擴展以下文字，增加更多細節與例子，使內容更豐富。直接輸出擴展後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_extend_moderate')}</button>
                                                    <button onClick={() => handleAiImprove('請大幅擴展以下文字，加入豐富細節、具體說明與多個例子。直接輸出擴展後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_extend_large')}</button>
                                                </>
                                            )}
                                            {aiDropdownType === 'shorten' && (
                                                <>
                                                    <button onClick={() => setAiDropdownType('main')} className="w-full flex items-center gap-2 px-2 py-1 text-fs-xs text-text-tertiary hover:text-text-secondary rounded-md transition-colors mb-0.5">
                                                        <RotateCcw className="w-3 h-3 shrink-0" /> {t('editor.ai_back')}
                                                    </button>
                                                    <div className="px-2.5 py-1 text-[10px] font-medium text-text-tertiary">{t('editor.ai_shorten_degree')}</div>
                                                    <button onClick={() => handleAiImprove('請適度精簡以下文字，移除冗詞並保持原意。直接輸出精簡後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_shorten_slight')}</button>
                                                    <button onClick={() => handleAiImprove('請精簡以下文字，保留重點並刪除非必要細節。直接輸出精簡後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_shorten_moderate')}</button>
                                                    <button onClick={() => handleAiImprove('請大幅濃縮以下文字，壓縮成更精煉的幾句話。直接輸出精簡後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_shorten_large')}</button>
                                                </>
                                            )}
                                            {aiDropdownType === 'tone' && (
                                                <>
                                                    <button onClick={() => setAiDropdownType('main')} className="w-full flex items-center gap-2 px-2 py-1 text-fs-xs text-text-tertiary hover:text-text-secondary rounded-md transition-colors mb-0.5">
                                                        <RotateCcw className="w-3 h-3 shrink-0" /> {t('editor.ai_back')}
                                                    </button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成學術研究風格，使用嚴謹的學術語言與正式結構。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_academic')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成專業商務風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_business')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成輕鬆隨性的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_casual')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成適合兒童閱讀的風格，使用簡單詞彙。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_childfriendly')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成自信且肯定的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_confident')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成自然對話風格，像在與人交談。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_conversational')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成更有創意與想像力的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_creative')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成更有情感張力、能打動人心的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_emotional')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成充滿熱情與活力的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_excited')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成正式且嚴謹的書面風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_formal')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成親切友善的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_friendly')}</button>
                                                    <button onClick={() => handleAiImprove('請將以下文字改寫成幽默風趣的風格。直接輸出改寫後的文字，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_tone_funny')}</button>
                                                </>
                                            )}
                                            {aiDropdownType === 'translate' && (
                                                <>
                                                    <button onClick={() => setAiDropdownType('main')} className="w-full flex items-center gap-2 px-2 py-1 text-fs-xs text-text-tertiary hover:text-text-secondary rounded-md transition-colors mb-0.5">
                                                        <RotateCcw className="w-3 h-3 shrink-0" /> {t('editor.ai_back')}
                                                    </button>
                                                    <button onClick={() => handleAiImprove('將以下文字翻譯為繁體中文。直接輸出翻譯結果，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_translate_zhtw')}</button>
                                                    <button onClick={() => handleAiImprove('將以下文字翻譯為簡體中文。直接輸出翻譯結果，不要附上任何解釋：')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_translate_zhcn')}</button>
                                                    <button onClick={() => handleAiImprove('Translate the following text into English. Output only the translation, no explanations:')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_translate_en')}</button>
                                                    <button onClick={() => handleAiImprove('以下のテキストを日本語に翻訳して出力してください。翻訳文のみを出力してください。')} className="w-full px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">{t('editor.ai_translate_ja')}</button>
                                                </>
                                            )}
                                            {aiDropdownType === 'custom' && (
                                                <>
                                                    <button onClick={() => setAiDropdownType('main')} className="w-full flex items-center gap-2 px-2 py-1 text-fs-xs text-text-tertiary hover:text-text-secondary rounded-md transition-colors mb-0.5">
                                                        <RotateCcw className="w-3 h-3 shrink-0" /> {t('editor.ai_back')}
                                                    </button>
                                                    <div className="px-2 pb-1">
                                                        <p className="text-[10px] font-medium text-text-tertiary mb-1.5">{t('editor.ai_custom_label')}</p>
                                                        <textarea
                                                            autoFocus
                                                            value={customPrompt}
                                                            onChange={e => setCustomPrompt(e.target.value)}
                                                            onMouseDown={e => e.stopPropagation()}
                                                            onClick={e => e.stopPropagation()}
                                                            onKeyDown={e => {
                                                                if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && customPrompt.trim()) {
                                                                    e.preventDefault();
                                                                    handleAiImprove(customPrompt.trim());
                                                                }
                                                                e.stopPropagation();
                                                            }}
                                                            placeholder={t('editor.ai_custom_placeholder')}
                                                            rows={4}
                                                            className="w-full resize-none rounded-md border border-stroke-divider bg-surface-subtle text-fs-xs text-text-primary placeholder:text-text-tertiary px-2.5 py-2 focus:outline-none focus:ring-1 focus:ring-accent-default transition-colors"
                                                        />
                                                        <button
                                                            onClick={() => { if (customPrompt.trim()) handleAiImprove(customPrompt.trim()); }}
                                                            disabled={!customPrompt.trim()}
                                                            className="mt-2 w-full flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md bg-accent-default text-white text-fs-xs font-medium transition-opacity disabled:opacity-40 disabled:cursor-not-allowed hover:opacity-90"
                                                        >
                                                            <Sparkles className="w-3.5 h-3.5" /> {t('editor.ai_custom_submit')}
                                                        </button>
                                                        <p className="mt-1.5 text-[10px] text-text-tertiary text-center">按 Ctrl+Enter 送出</p>
                                                    </div>
                                                </>
                                            )}
                                        </div>
                                    )}
                                </div>

                                <div className="w-px h-4 bg-stroke-divider mx-1" />

                                {/* Turn Into Dropdown for BubbleMenu */}
                                <div className="relative" ref={bubbleTurnIntoRef}>
                                    <button
                                        onClick={() => setShowBubbleTurnInto(v => !v)}
                                        className={`flex items-center gap-1 px-2 py-1.5 rounded hover:bg-surface-subtle text-text-secondary transition-colors text-fs-xs font-medium ${showBubbleTurnInto ? 'bg-surface-subtle' : ''}`}
                                        title={t('turn_into.label')}
                                    >
                                        <span>
                                            {editor.isActive('heading', { level: 1 }) ? t('turn_into.heading1')
                                            : editor.isActive('heading', { level: 2 }) ? t('turn_into.heading2')
                                            : editor.isActive('heading', { level: 3 }) ? t('turn_into.heading3')
                                            : editor.isActive('heading', { level: 4 }) ? t('turn_into.heading4')
                                            : editor.isActive('heading', { level: 5 }) ? t('turn_into.heading5')
                                            : editor.isActive('heading', { level: 6 }) ? t('turn_into.heading6')
                                            : t('turn_into.text')}
                                        </span>
                                        <ChevronDown className={`w-3 h-3 transition-transform ${showBubbleTurnInto ? 'rotate-180' : ''}`} />
                                    </button>
                                    {showBubbleTurnInto && (
                                        <div
                                            className="absolute left-0 top-full mt-1 w-44 bg-surface-base border border-stroke-divider rounded-lg shadow-xl p-1 z-[110] animate-in fade-in slide-in-from-top-1 duration-200"
                                            onMouseDown={e => e.stopPropagation()}
                                        >
                                            {([
                                                { key: 'text', label: t('turn_into.text'), icon: 'T', action: () => editor.chain().focus().setParagraph().run(), isActive: editor.isActive('paragraph') && !editor.isActive('heading') },
                                                { key: 'h1', label: t('turn_into.heading1'), icon: 'H1', action: () => editor.chain().focus().setHeading({ level: 1 }).run(), isActive: editor.isActive('heading', { level: 1 }) },
                                                { key: 'h2', label: t('turn_into.heading2'), icon: 'H2', action: () => editor.chain().focus().setHeading({ level: 2 }).run(), isActive: editor.isActive('heading', { level: 2 }) },
                                                { key: 'h3', label: t('turn_into.heading3'), icon: 'H3', action: () => editor.chain().focus().setHeading({ level: 3 }).run(), isActive: editor.isActive('heading', { level: 3 }) },
                                                { key: 'h4', label: t('turn_into.heading4'), icon: 'H4', action: () => editor.chain().focus().setHeading({ level: 4 }).run(), isActive: editor.isActive('heading', { level: 4 }) },
                                                { key: 'h5', label: t('turn_into.heading5'), icon: 'H5', action: () => editor.chain().focus().setHeading({ level: 5 }).run(), isActive: editor.isActive('heading', { level: 5 }) },
                                                { key: 'h6', label: t('turn_into.heading6'), icon: 'H6', action: () => editor.chain().focus().setHeading({ level: 6 }).run(), isActive: editor.isActive('heading', { level: 6 }) },
                                            ] as const).map(item => (
                                                <button
                                                    key={item.key}
                                                    onClick={() => { item.action(); setShowBubbleTurnInto(false); }}
                                                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium rounded-md transition-colors text-left ${item.isActive ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                                                >
                                                    <span className="w-5 text-center text-[10px] font-bold shrink-0 text-text-tertiary">{item.icon}</span>
                                                    {item.label}
                                                </button>
                                            ))}
                                        </div>
                                    )}
                                </div>

                                <div className="w-px h-4 bg-stroke-divider mx-1" />

                                <button
                                    onClick={() => editor.chain().focus().toggleBold().run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="粗體"
                                >
                                    <Bold className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => editor.chain().focus().toggleItalic().run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="斜體"
                                >
                                    <Italic className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => editor.chain().focus().toggleUnderline().run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="底線"
                                >
                                    <UnderlineIcon className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => editor.chain().focus().toggleHighlight().run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                                                        title="螢光筆"
                                >
                                    <Highlighter className="w-4 h-4" />
                                </button>

                                <div className="w-px h-4 bg-stroke-divider mx-1" />

                                {/* Bullet List Button */}
                                <button
                                    onClick={() => editor.chain().focus().toggleBulletList().run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title={t('list_dropdown.bullet_list')}
                                >
                                    <List className="w-4 h-4" />
                                </button>
                                {/* Ordered List Button */}
                                <button
                                    onClick={() => editor.chain().focus().toggleOrderedList().run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title={t('list_dropdown.ordered_list')}
                                >
                                    <ListOrdered className="w-4 h-4" />
                                </button>

                                <div className="w-px h-4 bg-stroke-divider mx-1" />
                                {/* Text Align Buttons */}
                                <button
                                    onClick={() => editor.chain().focus().setTextAlign('left').run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="靠左對齊"
                                >
                                    <AlignLeft className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => editor.chain().focus().setTextAlign('center').run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="置中對齊"
                                >
                                    <AlignCenter className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => editor.chain().focus().setTextAlign('right').run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="靠右對齊"
                                >
                                    <AlignRight className="w-4 h-4" />
                                </button>
                                <button
                                    onClick={() => editor.chain().focus().setTextAlign('justify').run()}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="兩端對齊"
                                >
                                    <AlignJustify className="w-4 h-4" />
                                </button>

                                <div className="w-px h-4 bg-stroke-divider mx-1" />
                                <button
                                    onClick={() => {
                                        const url = window.prompt('請輸入網址:');
                                        if (url) {
                                            editor.chain().focus().setLink({ href: url }).run();
                                        }
                                    }}
                                    className="p-1.5 rounded hover:bg-surface-subtle transition-colors text-text-secondary"
                                    title="超連結"
                                >
                                    <LinkIcon className="w-4 h-4" />
                                </button>
                            </div>
                        </div>
                </div>
            )}

            {/* Table Fixed Menu ???��??�表?��??�置中�?不用 BubbleMenu */}
            {/* 顯示條件：�?標在表格?��?point selection）�?�?cell ?��?（CellSelection，用?��?併�?位�? */}
            {editor && tableMenuPos && (
                editor.state.selection.from === editor.state.selection.to
                || !!(editor.state.selection as any).$anchorCell
            ) && (
                <div
                    style={{
                        position: 'fixed',
                        left: `${tableMenuPos.x}px`,
                        top: `${tableMenuPos.y - 8}px`,
                        transform: 'translate(-50%, -100%)',
                        zIndex: 200,
                    }}
                    onMouseDown={e => e.stopPropagation()}
                    className="flex items-center gap-0.5 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl px-2 py-1.5 animate-in fade-in zoom-in duration-200"
                >
                    {/* Columns */}
                    <button
                        onClick={() => editor.chain().focus().addColumnBefore().run()}
                        className="p-1.5 rounded hover:bg-accent-light2 text-text-secondary hover:text-accent-default transition-colors flex items-center gap-0.5"
                        title="左側加一列"
                    >
                        <Plus className="w-3 h-3" />
                        <Columns className="w-4 h-4 rotate-180" />
                    </button>
                    <button
                        onClick={() => editor.chain().focus().addColumnAfter().run()}
                        className="p-1.5 rounded hover:bg-accent-light2 text-text-secondary hover:text-accent-default transition-colors flex items-center gap-0.5"
                        title="右側加一列"
                    >
                        <Columns className="w-4 h-4" />
                        <Plus className="w-3 h-3" />
                    </button>
                    <button
                        onClick={() => editor.chain().focus().deleteColumn().run()}
                        className="p-1.5 rounded hover:bg-status-error/10 text-status-error transition-colors"
                        title="刪除列"
                    >
                        <Trash2 className="w-4 h-4" />
                    </button>

                    <div className="w-px h-4 bg-stroke-divider mx-1" />

                    {/* Rows */}
                    <button
                        onClick={() => editor.chain().focus().addRowBefore().run()}
                        className="p-1.5 rounded hover:bg-accent-light2 text-text-secondary hover:text-accent-default transition-colors flex items-center gap-0.5"
                        title="後面加一行"
                    >
                        <Plus className="w-3 h-3" />
                        <LayoutTemplate className="w-4 h-4 -rotate-90" />
                    </button>
                    <button
                        onClick={() => editor.chain().focus().addRowAfter().run()}
                        className="p-1.5 rounded hover:bg-accent-light2 text-text-secondary hover:text-accent-default transition-colors flex items-center gap-0.5"
                        title="前面加一行"
                    >
                        <LayoutTemplate className="w-4 h-4 rotate-90" />
                        <Plus className="w-3 h-3" />
                    </button>
                    <button
                        onClick={() => editor.chain().focus().deleteRow().run()}
                        className="p-1.5 rounded hover:bg-status-error/10 text-status-error transition-colors"
                                                title="刪除行"
                    >
                        <Trash2 className="w-4 h-4" />
                    </button>

                    <div className="w-px h-4 bg-stroke-divider mx-1" />

                    {/* Cell Merge/Split */}
                    <button
                        onClick={() => editor.chain().focus().mergeCells().run()}
                        className="p-1.5 rounded hover:bg-surface-subtle text-text-secondary transition-colors"
                                                title="合併格"
                    >
                        <Merge className="w-4 h-4" />
                    </button>
                    <button
                        onClick={() => editor.chain().focus().splitCell().run()}
                        className="p-1.5 rounded hover:bg-surface-subtle text-text-secondary transition-colors"
                                                title="分割格"
                    >
                        <Split className="w-4 h-4" />
                    </button>

                    <div className="w-px h-4 bg-stroke-divider mx-1" />

                    {/* Delete Table */}
                    <button
                        onClick={() => editor.chain().focus().deleteTable().run()}
                        className="p-1.5 rounded hover:bg-status-error/10 text-status-error transition-colors"
                                                title="刪除整個表格"
                    >
                        <Trash2 className="w-4 h-4" />
                    </button>
                </div>
            )}

            {/* Editor Content Area */}
            <div className="flex-1 overflow-y-auto">
                <EditorContent editor={editor} />
            </div>

            {/* AI Result Panel - position fixed, independent of BubbleMenu */}
            {(isAiImproving || aiImproveResult) && aiPanelAnchor && (
                <div ref={aiPanelRef} style={calcPanelStyle(aiPanelAnchor)}
                    className="bg-surface-base border border-stroke-divider rounded-lg shadow-2xl animate-in fade-in slide-in-from-bottom-2 duration-200">
                    <div className="flex flex-col">
                        {/* Header */}
                        <div className="flex items-center justify-between px-3 py-2 border-b border-stroke-divider">
                            <div className="flex items-center gap-1.5 text-accent-default">
                                {isAiImproving ? (
                                    <div className="w-3.5 h-3.5 border-2 border-accent-default border-t-transparent rounded-full animate-spin shrink-0" />
                                ) : (
                                    <Sparkles className="w-3.5 h-3.5" />
                                )}
                                <span className="text-fs-xs font-bold font-heading">
                                    {isAiImproving ? t('editor.ai_improving') : t('editor.ai_result_title')}
                                </span>
                            </div>
                            {!isAiImproving && (
                                <button onClick={() => { setAiImproveResult(null); setAiPanelAnchor(null); }}
                                    className="p-0.5 rounded hover:bg-surface-subtle text-text-tertiary transition-colors">
                                    <X className="w-3.5 h-3.5" />
                                </button>
                            )}
                        </div>
                        {/* Result text (shown during streaming and after) */}
                        {(aiImproveResult !== null && aiImproveResult !== '') && (
                            <div className="px-3 py-2.5 text-fs-sm text-text-primary leading-relaxed max-h-[200px] overflow-y-auto italic">
                                {aiImproveResult}
                                {isAiImproving && <span className="inline-block w-0.5 h-3.5 bg-accent-default animate-pulse ml-0.5 align-middle" />}
                            </div>
                        )}
                        {/* Empty streaming placeholder */}
                        {isAiImproving && (aiImproveResult === null || aiImproveResult === '') && (
                            <div className="px-3 py-3 flex items-center gap-2">
                                <div className="flex gap-1">
                                    <span className="w-1.5 h-1.5 bg-accent-default rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                                    <span className="w-1.5 h-1.5 bg-accent-default rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                                    <span className="w-1.5 h-1.5 bg-accent-default rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                                </div>
                            </div>
                        )}
                        {/* Action buttons - only after streaming complete */}
                        {!isAiImproving && aiImproveResult && (
                            <div className="flex items-center gap-2 px-3 py-2 border-t border-stroke-divider">
                                <button onClick={() => {
                                    if (!editor || !aiImproveResult) return;
                                    const { from, to } = editor.state.selection;
                                    if (from !== to) {
                                        editor.chain().focus().deleteSelection().insertContent(aiImproveResult).run();
                                    } else {
                                        editor.chain().focus().insertContent(aiImproveResult).run();
                                    }
                                    setAiImproveResult(null);
                                    setAiPanelAnchor(null);
                                }}
                                    className="flex items-center gap-1 px-2.5 py-1.5 bg-accent-default text-white rounded-md text-[10px] font-medium hover:bg-accent-dark transition-colors">
                                    <Check className="w-3 h-3" /> {t('editor.ai_apply')}
                                </button>
                                <button onClick={() => {
                                    if (lastPromptRef.current) {
                                        setAiImproveResult(null);
                                        handleAiImprove(lastPromptRef.current);
                                    }
                                }}
                                    className="flex items-center gap-1 px-2.5 py-1.5 bg-surface-subtle text-text-secondary rounded-md text-[10px] font-medium hover:bg-surface-elevated transition-colors">
                                    <RefreshCw className="w-3 h-3" /> {t('editor.ai_retry')}
                                </button>
                                <button onClick={() => { setAiImproveResult(null); setAiPanelAnchor(null); }}
                                    className="flex items-center gap-1 px-2.5 py-1.5 bg-surface-subtle text-text-secondary rounded-md text-[10px] font-medium hover:bg-surface-elevated transition-colors">
                                    <RotateCcw className="w-3 h-3" /> {t('editor.ai_discard')}
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            )}

            {/* Footer */}
            <div className="shrink-0 h-8 border-t border-stroke-divider px-4 flex items-center justify-between text-fs-xs text-text-tertiary bg-surface-subtle" onClick={(e) => e.stopPropagation()}>
                <span>字數 {editor?.storage.characterCount?.words() || 0}</span>
                <div className="flex items-center gap-2">
                    <span>已儲存</span>
                    <div className="relative" ref={historyMenuRef}>
                        <button
                            type="button"
                            onClick={() => { setHistoryEntries(loadHistory(activeTabId)); setShowHistoryMenu(v => !v); }}
                            className={`p-1 rounded transition-colors ${showHistoryMenu ? 'bg-surface-base text-text-primary' : 'text-text-tertiary hover:bg-surface-base hover:text-text-secondary'}`}
                            title="歷史快照（最多 20 個）"
                        >
                            <History className="w-3.5 h-3.5" />
                        </button>
                        {showHistoryMenu && (
                            <div className="absolute bottom-full right-0 mb-1 w-64 bg-surface-base border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1.5 ring-1 ring-black/5 animate-in fade-in zoom-in duration-150">
                                <div className="px-3 py-1.5 text-[10px] font-medium text-text-tertiary border-b border-stroke-divider mb-1">
                                    歷史快照（{tabs.find(t => t.id === activeTabId)?.title}）
                                </div>
                                {historyEntries.length === 0 ? (
                                    <div className="px-3 py-3 text-fs-xs text-text-tertiary text-center">暫無歷史紀錄<br /><span className="text-[10px]">每 5 分鐘自動建立快照</span></div>
                                ) : (
                                    <div className="max-h-60 overflow-y-auto">
                                        {historyEntries.map((entry, i) => (
                                            <button
                                                key={entry.ts}
                                                onClick={() => restoreHistory(entry)}
                                                className="w-full flex items-center justify-between px-3 py-2 text-fs-xs hover:bg-surface-subtle transition-colors text-left rounded-md"
                                            >
                                                <span className="text-text-secondary font-medium">{formatTs(entry.ts)}</span>
                                                <span className="text-text-tertiary text-[10px]">快照 #{historyEntries.length - i}</span>
                                            </button>
                                        ))}
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
};
