import React, { useState, useRef, useEffect, useCallback } from 'react';

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { tauriCmd } from '../../lib/tauri';
import { useEditor, EditorContent } from '@tiptap/react';
import { BubbleMenu } from '@tiptap/react/menus';
import StarterKit from '@tiptap/starter-kit';
import Paragraph from '@tiptap/extension-paragraph';
import Placeholder from '@tiptap/extension-placeholder';
import Highlight from '@tiptap/extension-highlight';
import Underline from '@tiptap/extension-underline';
import TextAlign from '@tiptap/extension-text-align';
import Link from '@tiptap/extension-link';
import { Table, TableRow, TableCell, TableHeader } from '@tiptap/extension-table';
import BubbleMenuExtension from '@tiptap/extension-bubble-menu';
import { Node as TiptapNode, Extension, mergeAttributes, nodeInputRule } from '@tiptap/core';
import type { EditorValidationIssue } from '../../lib/editor-validation';

import { ReactNodeViewRenderer } from '@tiptap/react';
import ImageNodeView from './extensions/ImageNodeView';

const inputRegex = /(?:^|\s)(!\[(.+|:?)]\((\S+)(?:(?:\s+)["'](\S+)["'])?\))$/;

const ParagraphVariant = Paragraph.extend({
    addAttributes() {
        return {
            ...this.parent?.(),
            variant: {
                default: 'text1',
                parseHTML: element => element.getAttribute('data-variant') || 'text1',
                renderHTML: attributes => {
                    const variant = attributes.variant || 'text1';
                    return variant !== 'text1' ? { 'data-variant': variant } : {};
                },
            },
        };
    },
});

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
    Bold, Italic, Underline as UnderlineIcon,
    List, ListOrdered,
    Code, Highlighter, X, Table as TableIcon, Plus, Trash2,
    AlignLeft, AlignCenter, AlignRight, AlignJustify,
    Upload, Columns, Merge, Split, LayoutTemplate, ChevronDown, Link as LinkIcon, Image as ImageIcon,
    Sparkles, Wand2, Eraser, Check, RotateCcw, RefreshCw, FileText, Languages, Smile, ChevronRight,
    FolderOpen, History as HistoryIcon, PanelLeftClose, PanelLeftOpen,
    Briefcase, Shield, Coffee, Maximize, Minimize, Eye, ClipboardCheck, AlertTriangle, Loader2,
} from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { getEnabledEditorAiActions } from '../../lib/editor-ai-actions';
import {
    buildStandaloneHtml,
    htmlToMarkdown,
    writeDocxFromHtml,
    writePdfFromElement,
    type EditorExportFormat,
} from '../../lib/editor-export';
import { validateEditorDocument } from '../../lib/editor-validation';
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

interface TableAction {
    key: string;
    label: string;
    icon: any;
    onClick: () => void;
    disabled?: boolean;
    danger?: boolean;
}

interface TableActionGroup {
    key: string;
    label: string;
    actions: TableAction[];
}

function getTableActionGroups(editor: any, t: (key: string) => string): TableActionGroup[] {
    return [
        {
            key: 'rows-columns',
            label: t('editor.rows_and_columns'),
            actions: [
                { key: 'add-row-before', icon: Plus, label: t('editor.add_row_above'), onClick: () => editor.chain().focus().addRowBefore().run() },
                { key: 'add-row-after', icon: Plus, label: t('editor.add_row_below'), onClick: () => editor.chain().focus().addRowAfter().run() },
                { key: 'add-column-before', icon: Columns, label: t('editor.add_column_left'), onClick: () => editor.chain().focus().addColumnBefore().run() },
                { key: 'add-column-after', icon: Columns, label: t('editor.add_column_right'), onClick: () => editor.chain().focus().addColumnAfter().run() },
            ],
        },
        {
            key: 'table-actions',
            label: t('editor.table_actions'),
            actions: [
                { key: 'merge-cells', icon: Merge, label: t('editor.merge_cells'), onClick: () => editor.chain().focus().mergeCells().run(), disabled: !editor.can().mergeCells() },
                { key: 'split-cell', icon: Split, label: t('editor.split_cell'), onClick: () => editor.chain().focus().splitCell().run(), disabled: !editor.can().splitCell() },
                { key: 'toggle-header-row', icon: LayoutTemplate, label: t('editor.toggle_header_row'), onClick: () => editor.chain().focus().toggleHeaderRow().run() },
            ],
        },
        {
            key: 'delete',
            label: '',
            actions: [
                { key: 'delete-row', icon: Trash2, label: t('editor.delete_this_row'), onClick: () => editor.chain().focus().deleteRow().run(), danger: true },
                { key: 'delete-column', icon: Trash2, label: t('editor.delete_this_column'), onClick: () => editor.chain().focus().deleteColumn().run(), danger: true },
                { key: 'delete-table', icon: Trash2, label: t('editor.delete_table'), onClick: () => editor.chain().focus().deleteTable().run(), danger: true },
            ],
        },
    ];
}

const MenuBar = React.memo(({ editor, fileName, onOpenDocument }: MenuBarProps) => {
    const t = useT();
    const [exportState, setExportState] = useState<'idle' | 'running' | 'success' | 'error'>('idle');
    const [exportMessage, setExportMessage] = useState('');
    const [previewHtml, setPreviewHtml] = useState<string | null>(null);
    const [showValidationPanel, setShowValidationPanel] = useState(false);
    const [validationIssues, setValidationIssues] = useState<EditorValidationIssue[]>([]);
    const menuStatesRef = useRef({
        bold: false,
        italic: false,
        underline: false,
        highlight: false,
        text1: false,
        text2: false,
        text3: false,
        h1: false,
        h2: false,
        h3: false,
        h4: false,
        bulletList: false,
        orderedList: false,
        alignLeft: false,
        alignCenter: false,
        alignRight: false,
        alignJustify: false,
        link: false,
        table: false,
        codeBlock: false,
    });
    const [menuStates, setMenuStates] = useState(menuStatesRef.current);
    useEffect(() => {
        if (!editor) return;
        const updateActiveStates = () => {
            const newStates = {
                bold: editor.isActive('bold'),
                italic: editor.isActive('italic'),
                underline: editor.isActive('underline'),
                highlight: editor.isActive('highlight'),
                text1: editor.isActive('paragraph', { variant: 'text1' }),
                text2: editor.isActive('paragraph', { variant: 'text2' }),
                text3: editor.isActive('paragraph', { variant: 'text3' }),
                h1: editor.isActive('heading', { level: 1 }),
                h2: editor.isActive('heading', { level: 2 }),
                h3: editor.isActive('heading', { level: 3 }),
                h4: editor.isActive('heading', { level: 4 }),
                bulletList: editor.isActive('bulletList'),
                orderedList: editor.isActive('orderedList'),
                alignLeft: editor.isActive({ textAlign: 'left' }),
                alignCenter: editor.isActive({ textAlign: 'center' }),
                alignRight: editor.isActive({ textAlign: 'right' }),
                alignJustify: editor.isActive({ textAlign: 'justify' }),
                link: editor.isActive('link'),
                table: editor.isActive('table'),
                codeBlock: editor.isActive('codeBlock'),
            };
            if (JSON.stringify(menuStatesRef.current) !== JSON.stringify(newStates)) {
                menuStatesRef.current = newStates;
                setMenuStates(newStates);
            }
        };
        updateActiveStates();
        editor.on('update', updateActiveStates);
        editor.on('transaction', updateActiveStates);
        editor.on('selectionUpdate', updateActiveStates);
        return () => {
            editor.off('update', updateActiveStates);
            editor.off('transaction', updateActiveStates);
            editor.off('selectionUpdate', updateActiveStates);
        };
    }, [editor]);
    const [showTableMenu, setShowTableMenu] = useState(false);
    const tableMenuRef = useRef<HTMLDivElement>(null);
    const [showExportMenu, setShowExportMenu] = useState(false);
    const exportMenuRef = useRef<HTMLDivElement>(null);
    const [showTurnIntoMenu, setShowTurnIntoMenu] = useState(false);
    const turnIntoMenuRef = useRef<HTMLDivElement>(null);

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
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    if (!editor) return null;
    const tableActionGroups = getTableActionGroups(editor, t);

    const ToolbarButton = ({ onClick, isActive = false, disabled = false, icon: Icon, title, label }: any) => (
        <button
            onMouseDown={(e) => e.preventDefault()}
            onClick={onClick}
            disabled={disabled}
            title={title}
            aria-pressed={isActive}
            className={`p-1.5 rounded transition-colors flex items-center gap-1.5 ${isActive ? 'bg-accent-light2 text-accent-default ring-1 ring-accent-default/30 font-medium' : 'text-text-secondary hover:bg-surface-subtle'} ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
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

    const issueText = (issue: EditorValidationIssue) => {
        const count = issue.count ?? 0;
        const key = `editor.validation_${issue.code}` as any;
        return t(key, { count });
    };

    const runValidation = () => {
        const issues = validateEditorDocument({
            title: fileName || '',
            html: editor.getHTML(),
            text: editor.getText(),
        });
        setValidationIssues(issues);
        setShowValidationPanel(true);
    };

    const openPreview = () => {
        setPreviewHtml(buildStandaloneHtml(editor.getHTML()));
    };

    const exportAs = async (format: EditorExportFormat) => {
        setShowExportMenu(false);
        const name = fileName || 'Document';
        const html = editor.getHTML();
        const text = editor.getText();

        const filterMap: Record<string, { name: string; extensions: string[] }[]> = {
            txt: [{ name: 'Plain Text', extensions: ['txt'] }],
            md: [{ name: 'Markdown', extensions: ['md'] }],
            html: [{ name: 'HTML Document', extensions: ['html'] }],
            docx: [{ name: 'Word Document', extensions: ['docx'] }],
            pdf: [{ name: 'PDF Document', extensions: ['pdf'] }],
        };
        const filePath = await saveDialog({
            defaultPath: `${name}.${format}`,
            filters: filterMap[format] ?? [],
        });
        if (!filePath) return; // User canceled save

        setExportState('running');
        setExportMessage(t('editor.export_running'));
        try {
            if (format === 'txt') {
                await tauriCmd.exportDocument(filePath, text);
            } else if (format === 'md') {
                await tauriCmd.exportDocument(filePath, await htmlToMarkdown(html));
            } else if (format === 'html') {
                await tauriCmd.exportDocument(filePath, buildStandaloneHtml(html));
            } else if (format === 'docx') {
                await writeDocxFromHtml(filePath, html);
            } else if (format === 'pdf') {
                const editorEl = document.querySelector('.ProseMirror') as HTMLElement;
                if (!editorEl) throw new Error('Editor element not found');
                await writePdfFromElement(filePath, editorEl);
            }

            const title = fileName || 'Document';
            const md = format === 'md'
                ? await htmlToMarkdown(html)
                : format === 'txt' ? text : html;
            try {
                await tauriCmd.saveEditorToKnowledge(title, md);
            } catch (error) {
                console.error('Save to repository failed:', error);
            }
            setExportState('success');
            setExportMessage(t('editor.export_success'));
            window.setTimeout(() => setExportState('idle'), 3000);
        } catch (error) {
            console.error('Export failed:', error);
            setExportState('error');
            setExportMessage(t('editor.export_failed'));
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
                title={t('editor.open_document')}
            >
                <FolderOpen className="w-3.5 h-3.5" />
                <span>{t('editor.open')}</span>
            </button>
            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <div className="relative">
                <button
                    type="button"
                    onClick={(e) => { e.stopPropagation(); runValidation(); }}
                    className={`flex items-center gap-1 px-2 py-1.5 rounded text-fs-xs font-medium transition-colors ${showValidationPanel ? 'bg-surface-subtle text-text-primary' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                    title={t('editor.document_check')}
                >
                    <ClipboardCheck className="w-3.5 h-3.5" />
                    <span>{t('editor.check')}</span>
                </button>
                {showValidationPanel && (
                    <div className="absolute left-0 top-full mt-1 w-80 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl z-[120] py-2 animate-in fade-in zoom-in duration-150">
                        <div className="flex items-center justify-between px-3 pb-2 border-b border-stroke-divider">
                            <span className="text-fs-xs font-semibold text-text-primary">{t('editor.document_check')}</span>
                            <button type="button" onClick={() => setShowValidationPanel(false)} className="p-0.5 rounded text-text-tertiary hover:bg-surface-subtle">
                                <X className="w-3.5 h-3.5" />
                            </button>
                        </div>
                        {validationIssues.length === 0 ? (
                            <div className="px-3 py-3 text-fs-xs text-emerald-500">{t('editor.validation_passed')}</div>
                        ) : (
                            <div className="max-h-72 overflow-y-auto px-2 py-2 space-y-1">
                                {validationIssues.map((issue, index) => (
                                    <div key={`${issue.code}-${index}`} className="flex gap-2 rounded-md px-2 py-1.5 text-fs-xs text-text-secondary hover:bg-surface-subtle">
                                        <AlertTriangle className={`mt-0.5 h-3.5 w-3.5 shrink-0 ${issue.severity === 'warning' ? 'text-amber-500' : 'text-sky-500'}`} />
                                        <span>{issueText(issue)}</span>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                )}
            </div>

            <button
                type="button"
                onClick={(e) => { e.stopPropagation(); openPreview(); }}
                className="flex items-center gap-1 px-2 py-1.5 rounded text-fs-xs font-medium transition-colors text-text-secondary hover:bg-surface-subtle hover:text-text-primary"
                title={t('editor.export_preview')}
            >
                <Eye className="w-3.5 h-3.5" />
                <span>{t('editor.preview')}</span>
            </button>
            <div className="w-px h-5 bg-stroke-divider mx-1" />

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
                    <div className="absolute left-0 top-full mt-1 w-40 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1.5 px-1.5 animate-in fade-in zoom-in duration-150">
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
            {exportState !== 'idle' && (
                <span className={`ml-1 inline-flex items-center gap-1 rounded-full px-2 py-1 text-[10px] font-medium ${exportState === 'error'
                    ? 'bg-red-500/10 text-red-500'
                    : exportState === 'success'
                        ? 'bg-emerald-500/10 text-emerald-500'
                        : 'bg-accent-default/10 text-accent-default'
                    }`}>
                    {exportState === 'running' && <Loader2 className="w-3 h-3 animate-spin" />}
                    {exportMessage}
                </span>
            )}
            {previewHtml && (
                <div className="fixed inset-0 z-[250] flex items-center justify-center bg-black/55 p-6" onMouseDown={(e) => e.stopPropagation()}>
                    <div className="flex h-[82vh] w-full max-w-5xl flex-col overflow-hidden rounded-xl border border-stroke-divider bg-surface-base shadow-2xl">
                        <div className="flex items-center justify-between border-b border-stroke-divider px-4 py-3">
                            <div>
                                <h3 className="text-fs-sm font-semibold text-text-primary">{t('editor.export_preview')}</h3>
                                <p className="text-fs-xs text-text-tertiary">{t('editor.preview_hint')}</p>
                            </div>
                            <button type="button" onClick={() => setPreviewHtml(null)} className="rounded p-1 text-text-tertiary hover:bg-surface-subtle hover:text-text-primary">
                                <X className="w-4 h-4" />
                            </button>
                        </div>
                        <iframe
                            title={t('editor.export_preview')}
                            srcDoc={previewHtml}
                            className="h-full w-full border-0 bg-white"
                        />
                    </div>
                </div>
            )}
            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <div className="relative" ref={turnIntoMenuRef}>
                {(() => {
                    const currentLabel = menuStates.h1 ? t('turn_into.heading1')
                        : menuStates.h2 ? t('turn_into.heading2')
                            : menuStates.h3 ? t('turn_into.heading3')
                                : menuStates.h4 ? t('turn_into.heading4')
                                    : menuStates.text2 ? t('turn_into.text2')
                                        : menuStates.text3 ? t('turn_into.text3')
                                            : t('turn_into.text1');
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
                    <div className="absolute left-0 top-full mt-1 w-44 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1 px-1 animate-in fade-in zoom-in duration-150">
                        {([
                            { key: 'text1', label: t('turn_into.text1'), icon: 'T1', action: () => editor.chain().focus().setParagraph().updateAttributes('paragraph', { variant: 'text1' }).run(), isActive: menuStates.text1 },
                            { key: 'text2', label: t('turn_into.text2'), icon: 'T2', action: () => editor.chain().focus().setParagraph().updateAttributes('paragraph', { variant: 'text2' }).run(), isActive: menuStates.text2 },
                            { key: 'text3', label: t('turn_into.text3'), icon: 'T3', action: () => editor.chain().focus().setParagraph().updateAttributes('paragraph', { variant: 'text3' }).run(), isActive: menuStates.text3 },
                            { key: 'h1', label: t('turn_into.heading1'), icon: 'H1', action: () => editor.chain().focus().setHeading({ level: 1 }).run(), isActive: menuStates.h1 },
                            { key: 'h2', label: t('turn_into.heading2'), icon: 'H2', action: () => editor.chain().focus().setHeading({ level: 2 }).run(), isActive: menuStates.h2 },
                            { key: 'h3', label: t('turn_into.heading3'), icon: 'H3', action: () => editor.chain().focus().setHeading({ level: 3 }).run(), isActive: menuStates.h3 },
                            { key: 'h4', label: t('turn_into.heading4'), icon: 'H4', action: () => editor.chain().focus().setHeading({ level: 4 }).run(), isActive: menuStates.h4 },
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
                title={t('editor.bold_shortcut')}
                onClick={() => editor.chain().focus().toggleBold().run()}
                isActive={menuStates.bold}
            />
            <ToolbarButton
                icon={Italic}
                title={t('editor.italic_shortcut')}
                onClick={() => editor.chain().focus().toggleItalic().run()}
                isActive={menuStates.italic}
            />
            <ToolbarButton
                icon={UnderlineIcon}
                title={t('editor.underline_shortcut')}
                onClick={() => editor.chain().focus().toggleUnderline().run()}
                isActive={menuStates.underline}
            />
            <ToolbarButton
                icon={Highlighter}
                title={t('editor.highlight')}
                onClick={() => editor.chain().focus().toggleHighlight().run()}
                isActive={menuStates.highlight}
            />
            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <ToolbarButton
                icon={AlignLeft}
                title={t('editor.align_left')}
                onClick={() => editor.chain().focus().setTextAlign('left').run()}
                isActive={menuStates.alignLeft}
            />
            <ToolbarButton
                icon={AlignCenter}
                title={t('editor.align_center')}
                onClick={() => editor.chain().focus().setTextAlign('center').run()}
                isActive={menuStates.alignCenter}
            />
            <ToolbarButton
                icon={AlignRight}
                title={t('editor.align_right')}
                onClick={() => editor.chain().focus().setTextAlign('right').run()}
                isActive={menuStates.alignRight}
            />
            <ToolbarButton
                icon={AlignJustify}
                title={t('editor.justify')}
                onClick={() => editor.chain().focus().setTextAlign('justify').run()}
                isActive={menuStates.alignJustify}
            />

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <ToolbarButton
                icon={List}
                title={t('list_dropdown.bullet_list')}
                onClick={() => editor.chain().focus().toggleBulletList().run()}
                isActive={menuStates.bulletList}
            />
            <ToolbarButton
                icon={ListOrdered}
                title={t('list_dropdown.ordered_list')}
                onClick={() => editor.chain().focus().toggleOrderedList().run()}
                isActive={menuStates.orderedList}
            />

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <div className="relative" ref={tableMenuRef}>
                <button
                    type="button"
                    onClick={(e) => {
                        e.stopPropagation();
                        if (menuStates.table) {
                            setShowTableMenu(!showTableMenu);
                        } else {
                            editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
                        }
                    }}
                    title={menuStates.table ? t('editor.table_actions') : t('editor.insert_table')}
                    className={`p-1.5 rounded transition-all flex items-center gap-0.5 ${menuStates.table
                        ? 'bg-accent-light2 text-accent-default shadow-sm'
                        : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'
                        }`}
                >
                    <TableIcon className="w-4 h-4" />
                    <ChevronDown className={`w-3 h-3 transition-transform duration-200 ${showTableMenu ? 'rotate-180' : ''}`} />
                </button>

                {showTableMenu && (
                    <div className="absolute left-0 top-full mt-1 w-56 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1.5 px-1.5 overflow-hidden animate-in fade-in zoom-in duration-150">
                        {tableActionGroups.map((group, groupIndex) => (
                            <React.Fragment key={group.key}>
                                {groupIndex > 0 && <div className="h-px bg-stroke-divider my-1.5 mx-1" />}
                                {group.label && (
                                    <div className="px-3 py-1.5 text-[10px] font-bold text-text-tertiary uppercase tracking-wider mb-1">
                                        {group.label}
                                    </div>
                                )}
                                {group.actions.map((action) => (
                                    <MenuAction
                                        key={action.key}
                                        icon={action.icon}
                                        label={action.label}
                                        onClick={action.onClick}
                                        disabled={action.disabled}
                                        danger={action.danger}
                                    />
                                ))}
                            </React.Fragment>
                        ))}
                    </div>
                )}
            </div>

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <label className="p-1.5 rounded transition-colors text-text-secondary hover:bg-surface-subtle cursor-pointer" title={t('editor.upload_image')}>
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

            <button
                onClick={() => {
                    const url = window.prompt(t('editor.prompt_enter_url'));
                    if (url) {
                        editor.chain().focus().setLink({ href: url }).run();
                    }
                }}
                className={`p-1.5 rounded transition-colors ${menuStates.link ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle hover:text-text-primary'}`}
                title={t('editor.hyperlink')}
            >
                <LinkIcon className="w-4 h-4" />
            </button>

            <div className="w-px h-5 bg-stroke-divider mx-1" />

            <ToolbarButton
                icon={Code}
                title={t('editor.code_block')}
                onClick={() => editor.chain().focus().toggleCodeBlock().run()}
                isActive={menuStates.codeBlock}
            />
        </div>
    );
});

function normalizeFsPath(path: string): string {
    return path.replace(/\\/g, '/').toLowerCase();
}

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
    const ICON_MAP: Record<string, any> = {
        Wand2, Smile, FileText, Eraser, Languages, RefreshCw, Sparkles,
        Briefcase, Shield, Coffee, Maximize, Minimize,
    };
    const [settings, setSettings] = useState<any>(null);
    useEffect(() => { invoke('get_settings').then(setSettings); }, []);
    const { isChatHidden, toggleChatHidden } = useUiStore();
    const t = useT();

    const [tabs, setTabs] = useState<EditorTab[]>(() => {
        const s = loadSession();
        if (s?.tabs?.length) return s.tabs.map(t => ({ id: t.id, title: t.title, content: '' }));
        return [{ id: 'tab-1', title: 'New Document 1', content: '' }];
    });
    const [activeTabId, setActiveTabId] = useState<string>(() => loadSession()?.activeTabId ?? 'tab-1');
    const tabContentsRef = useRef<Record<string, string>>((() => {
        const s = loadSession();
        const tabList = s?.tabs ?? [{ id: 'tab-1' }];
        const result: Record<string, string> = {};
        for (const t of tabList) result[t.id] = loadContent(t.id);
        return result;
    })());
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
                        this.editor.commands.sinkListItem('listItem');
                        return true;
                    }
                    this.editor.commands.insertContent('  ');
                    return true;
                },
                'Shift-Tab': () => {
                    if (this.editor.isActive('listItem')) {
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
            StarterKit.configure({
                paragraph: false,
            }),
            ParagraphVariant,
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
                placeholder: 'Write notes, reports, or drafts here...',
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

    const switchTab = useCallback((newTabId: string) => {
        if (!editor) return;
        const json = JSON.stringify(editor.getJSON());
        tabContentsRef.current[activeTabId] = json;
        saveContent(activeTabId, json);
        addHistory(activeTabId, json);
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
        const newTitle = `New Document ${tabCounter}`;
        const now = Date.now();
        upsertFile({ id: newId, title: newTitle, createdAt: now, updatedAt: now });
        tabContentsRef.current[newId] = '';
        setTabs(prev => [...prev, { id: newId, title: newTitle, content: '' }]);
        editor.commands.setContent('');
        setActiveTabId(newId);
    }, [editor, activeTabId]);

    const closeTab = useCallback((tabId: string, e: React.MouseEvent) => {
        e.stopPropagation();
        if (editor) {
            const closingJson = tabId === activeTabId
                ? JSON.stringify(editor.getJSON())
                : (tabContentsRef.current[tabId] ?? '');
            if (closingJson) saveContent(tabId, closingJson);
        }
        setTabs(prev => {
            if (prev.length <= 1) return prev; // Keep at least one tab
            const idx = prev.findIndex(t => t.id === tabId);
            const next = prev.filter(t => t.id !== tabId);
            delete tabContentsRef.current[tabId]; // Remove from memory, keep in localStorage
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
    const [isDragOver, setIsDragOver] = useState(false);

    const commitRename = useCallback((tabId: string) => {
        const trimmed = renameValue.trim();
        if (trimmed) {
            setTabs(prev => prev.map(t => t.id === tabId ? { ...t, title: trimmed } : t));
            renameNoteFile(tabId, trimmed);
        }
        setRenamingTabId(null);
    }, [renameValue]);


    useEffect(() => {
        if (!editor) return;
        const saved = tabContentsRef.current[activeTabId];
        if (saved) {
            try { editor.commands.setContent(JSON.parse(saved)); } catch { editor.commands.setContent(''); }
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [editor]); // run once when editor initialises

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

    useEffect(() => {
        saveSession({ activeTabId, tabs: tabs.map(t => ({ id: t.id, title: t.title })), tabCounter });
    }, [tabs, activeTabId]);

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

    useEffect(() => {
        if (!editor) return;
        const interval = setInterval(() => {
            addHistory(activeTabId, JSON.stringify(editor.getJSON()));
        }, 5 * 60 * 1000);
        return () => clearInterval(interval);
    }, [editor, activeTabId]);

    useEffect(() => {
        const now = Date.now();
        const existingIds = new Set(loadFiles().map(f => f.id));
        const toAdd = tabs.filter(t => !existingIds.has(t.id));
        if (toAdd.length > 0) {
            toAdd.forEach(t => upsertFile({ id: t.id, title: t.title, createdAt: now, updatedAt: now }));
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []); // only on mount


    const formatTs = (ts: number) => {
        const d = new Date(ts);
        return `${d.getMonth() + 1}/${d.getDate()} ${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
    };

    const openDocumentFromPath = useCallback((path: string, content: string) => {
        if (!editor) return;
        const normalizedTargetPath = normalizeFsPath(path);
        const existingTab = tabs.find(t => t.filePath && normalizeFsPath(t.filePath) === normalizedTargetPath);
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
        const title = path.split(/[\\/]/).pop() || 'Document';
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
                { name: 'Document', extensions: ['txt', 'md', 'markdown', 'html', 'htm'] },
            ],
        });

        if (!selected || Array.isArray(selected)) return;

        try {
            const content = await tauriCmd.openDocument(selected);
            openDocumentFromPath(selected, content);
        } catch (error) {
            console.error('Open document failed:', error);
            window.alert(t('editor.open_document_failed'));
        }
    }, [editor, openDocumentFromPath, t]);

    const restoreHistory = useCallback((entry: HistoryEntry) => {
        if (!editor) return;
        addHistory(activeTabId, JSON.stringify(editor.getJSON()));
        try {
            editor.commands.setContent(JSON.parse(entry.snapshot));
            tabContentsRef.current[activeTabId] = entry.snapshot;
            saveContent(activeTabId, entry.snapshot);
        } catch { }
        setShowHistoryMenu(false);
    }, [editor, activeTabId]);

    const [isAiImproving, setIsAiImproving] = useState(false);
    const [aiImproveResult, setAiImproveResult] = useState<string | null>(null);
    const [aiPanelAnchor, setAiPanelAnchor] = useState<{ x: number; y: number } | null>(null);
    const [showAiDropdown, setShowAiDropdown] = useState(false);
    const [aiDropdownType, setAiDropdownType] = useState<'main' | 'custom'>('main');
    const [customPrompt, setCustomPrompt] = useState('');
    const aiDropdownRef = useRef<HTMLDivElement>(null);
    const aiPanelRef = useRef<HTMLDivElement>(null);
    const [showBubbleTurnInto, setShowBubbleTurnInto] = useState(false);
    const bubbleTurnIntoRef = useRef<HTMLDivElement>(null);

    const lastPromptRef = useRef<string>('');
    const selectedTextRef = useRef<string>('');
    const selectedRangeRef = useRef<{ from: number; to: number } | null>(null);
    const aiTargetRangeRef = useRef<{ from: number; to: number } | null>(null);


    const popupStatesRef = useRef({
        bold: false,
        italic: false,
        underline: false,
        highlight: false,
        bulletList: false,
        orderedList: false,
        alignLeft: false,
        alignCenter: false,
        alignRight: false,
        alignJustify: false,
        link: false,
        imageAlignLeft: false,
        imageAlignCenter: false,
        imageAlignRight: false,
        imageWidth: '100%',
        tableActive: false,
    });
    const [popupStates, setPopupStates] = useState(popupStatesRef.current);

    useEffect(() => {
        if (!editor) return;

        const updatePopupStates = () => {
            const imageAttrs = editor.getAttributes('imageNodePro') as { width?: string };
            const newStates = {
                bold: editor.isActive('bold'),
                italic: editor.isActive('italic'),
                underline: editor.isActive('underline'),
                highlight: editor.isActive('highlight'),
                bulletList: editor.isActive('bulletList'),
                orderedList: editor.isActive('orderedList'),
                alignLeft: editor.isActive({ textAlign: 'left' }),
                alignCenter: editor.isActive({ textAlign: 'center' }),
                alignRight: editor.isActive({ textAlign: 'right' }),
                alignJustify: editor.isActive({ textAlign: 'justify' }),
                link: editor.isActive('link'),
                imageAlignLeft: editor.isActive('imageNodePro', { textAlign: 'left' }),
                imageAlignCenter: editor.isActive('imageNodePro', { textAlign: 'center' }),
                imageAlignRight: editor.isActive('imageNodePro', { textAlign: 'right' }),
                imageWidth: imageAttrs.width ?? '100%',
                tableActive: editor.isActive('table'),
            };

            if (JSON.stringify(popupStatesRef.current) !== JSON.stringify(newStates)) {
                popupStatesRef.current = newStates;
                setPopupStates(newStates);
            }
        };

        updatePopupStates();
        editor.on('update', updatePopupStates);
        editor.on('transaction', updatePopupStates);
        editor.on('selectionUpdate', updatePopupStates);

        return () => {
            editor.off('update', updatePopupStates);
            editor.off('transaction', updatePopupStates);
            editor.off('selectionUpdate', updatePopupStates);
        };
    }, [editor]);

    const [tableMenuPos, setTableMenuPos] = useState<{ x: number; y: number } | null>(null);

    useEffect(() => {
        if (!editor) return;
        let rafId: number | null = null;
        const update = () => {
            if (rafId !== null) return;
            rafId = requestAnimationFrame(() => {
                rafId = null;
                if (!editor.isActive('table')) {
                    setTableMenuPos(prev => prev === null ? prev : null);
                    return;
                }
                const { state, view } = editor;
                const { $from } = state.selection;
                let tPos: number | null = null;
                for (let d = $from.depth; d >= 0; d--) {
                    if ($from.node(d).type.name === 'table') {
                        tPos = $from.before(d);
                        break;
                    }
                }
                if (tPos !== null) {
                    const domNode = view.nodeDOM(tPos) as HTMLElement | null;
                    const tableEl = domNode?.nodeName === 'TABLE' ? domNode : domNode?.querySelector('table') ?? domNode;
                    if (tableEl) {
                        const rect = tableEl.getBoundingClientRect();
                        const nx = rect.left + rect.width / 2;
                        const ny = rect.top;
                        setTableMenuPos(prev => {
                            if (prev && Math.abs(prev.x - nx) < 1 && Math.abs(prev.y - ny) < 1) return prev;
                            return { x: nx, y: ny };
                        });
                    }
                }
            });
        };
        editor.on('selectionUpdate', update);
        editor.on('transaction', update);
        const scroller = editor.view.dom.closest('[data-editor-drop="true"]');
        scroller?.addEventListener('scroll', update, { passive: true });
        window.addEventListener('resize', update);
        update();
        return () => {
            editor.off('selectionUpdate', update);
            editor.off('transaction', update);
            scroller?.removeEventListener('scroll', update);
            window.removeEventListener('resize', update);
            if (rafId !== null) cancelAnimationFrame(rafId);
        };
    }, [editor]);

    const [textBubblePos, setTextBubblePos] = useState<{ x: number; y: number } | null>(null);
    const textBubbleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
        if (!editor) return;

        const showBubble = () => {
            if (textBubbleTimer.current) { clearTimeout(textBubbleTimer.current); textBubbleTimer.current = null; }
            textBubbleTimer.current = setTimeout(() => {
                if (!editor || isAiImproving || aiImproveResult) return;
                if ((editor.state.selection as any).$anchorCell) {
                    setTextBubblePos(null);
                    return;
                }
                const { from, to } = editor.state.selection;
                if (from === to || editor.isActive('imageNodePro')) {
                    setTextBubblePos(null);
                    return;
                }
                selectedRangeRef.current = { from, to };
                const { view } = editor;
                const start = view.coordsAtPos(from);
                const end = view.coordsAtPos(to);
                setTextBubblePos({ x: (start.left + end.right) / 2, y: Math.min(start.top, end.top) });
            }, 250);
        };

        const hideBubble = () => {
            if (textBubbleTimer.current) { clearTimeout(textBubbleTimer.current); textBubbleTimer.current = null; }
            const { from, to } = editor.state.selection;
            const isCellSelection = !!(editor.state.selection as any).$anchorCell;
            if (from === to || editor.isActive('imageNodePro') || isCellSelection) {
                setTextBubblePos(null);
            }
        };

        const editorDom = editor.view.dom;
        editorDom.addEventListener('mouseup', showBubble);
        editorDom.addEventListener('keyup', showBubble);
        editor.on('selectionUpdate', hideBubble);

        return () => {
            editorDom.removeEventListener('mouseup', showBubble);
            editorDom.removeEventListener('keyup', showBubble);
            editor.off('selectionUpdate', hideBubble);
            if (textBubbleTimer.current) clearTimeout(textBubbleTimer.current);
        };
    }, [editor, isAiImproving, aiImproveResult]);

    const handleAiImprove = useCallback(async (prompt: string) => {
        if (!editor || isAiImproving) return;

        let { from, to } = editor.state.selection;
        if (from === to && selectedRangeRef.current) {
            from = selectedRangeRef.current.from;
            to = selectedRangeRef.current.to;
        }
        const text = editor.state.doc.textBetween(from, to);
        if (!text.trim()) return;

        selectedRangeRef.current = { from, to };
        aiTargetRangeRef.current = { from, to };
        lastPromptRef.current = prompt;
        selectedTextRef.current = text;

        const coords = editor.view.coordsAtPos(from);
        setAiPanelAnchor(prev => prev ?? { x: coords.left, y: coords.top });

        setIsAiImproving(true);
        setAiImproveResult('');
        setShowAiDropdown(false);

        const convId = `ai-improve-${Date.now()}`;

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

                invoke('editor_ai_rewrite_stream', {
                    actionPrompt: prompt,
                    selectedText: text,
                    conversationId: convId,
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

    const floatingTableActionGroups = editor ? getTableActionGroups(editor, t) : [];

    return (
        <div className="w-full h-full flex flex-col border-l border-stroke-divider bg-surface-base" onClick={() => editor?.commands.focus()}>
            <div className="flex items-center border-b border-stroke-divider bg-surface-layer shrink-0 h-12" onClick={(e) => e.stopPropagation()}>
                <button
                    onClick={toggleChatHidden}
                    className={`px-2 self-stretch border-r border-stroke-divider text-text-tertiary hover:text-text-secondary hover:bg-surface-subtle transition-colors shrink-0 flex items-center ${isChatHidden ? 'text-accent-default' : ''}`}
                    title={isChatHidden ? 'Show chat' : 'Hide chat'}
                >
                    {isChatHidden ? <PanelLeftOpen className="w-4 h-4" /> : <PanelLeftClose className="w-4 h-4" />}
                </button>
                <div className="flex items-center min-w-0 flex-1 overflow-x-auto">
                    {tabs.map(tab => (
                        <div
                            key={tab.id}
                            onClick={() => tab.id !== activeTabId && switchTab(tab.id)}
                            onDoubleClick={() => { setRenamingTabId(tab.id); setRenameValue(tab.title); }}
                            className={`group flex items-center gap-1.5 px-3 py-2 text-fs-xs font-medium cursor-pointer shrink-0 border-r border-stroke-divider transition-colors select-none ${tab.id === activeTabId
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
                        title={t('editor.new_document')}
                    >
                        +
                    </button>
                </div>
            </div>

            <MenuBar
                editor={editor}
                fileName={tabs.find(t => t.id === activeTabId)?.title ?? 'Document'}
                onOpenDocument={handleOpenDocument}
            />

            {editor && (
                <BubbleMenu
                    editor={editor}
                    shouldShow={() => !!editor.isActive('imageNodePro') && !textBubblePos}
                    options={{
                        offset: 8,
                        placement: 'top',
                    }}
                >
                    <div className="flex items-center gap-0.5 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl px-2 py-1.5 animate-in fade-in zoom-in duration-200 z-[100] mx-6">
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { textAlign: 'left' }).run()}
                            className={`p-1.5 rounded hover:bg-surface-subtle transition-colors ${popupStates.imageAlignLeft ? 'text-accent-default bg-accent-light2' : 'text-text-secondary'}`}
                            title={t('editor.align_left')}
                        >
                            <AlignLeft className="w-4 h-4" />
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { textAlign: 'center' }).run()}
                            className={`p-1.5 rounded hover:bg-surface-subtle transition-colors ${popupStates.imageAlignCenter ? 'text-accent-default bg-accent-light2' : 'text-text-secondary'}`}
                            title={t('editor.align_center')}
                        >
                            <AlignCenter className="w-4 h-4" />
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { textAlign: 'right' }).run()}
                            className={`p-1.5 rounded hover:bg-surface-subtle transition-colors ${popupStates.imageAlignRight ? 'text-accent-default bg-accent-light2' : 'text-text-secondary'}`}
                            title={t('editor.align_right')}
                        >
                            <AlignRight className="w-4 h-4" />
                        </button>
                        <div className="w-px h-4 bg-stroke-divider mx-1" />
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { width: '25%' }).run()}
                            className={`px-1.5 py-1 text-[10px] font-bold rounded transition-colors ${popupStates.imageWidth === '25%' ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                        >
                            25%
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { width: '50%' }).run()}
                            className={`px-1.5 py-1 text-[10px] font-bold rounded transition-colors ${popupStates.imageWidth === '50%' ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                        >
                            50%
                        </button>
                        <button
                            onClick={() => editor.chain().focus().updateAttributes('imageNodePro', { width: '100%' }).run()}
                            className={`px-1.5 py-1 text-[10px] font-bold rounded transition-colors ${popupStates.imageWidth === '100%' ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                        >
                            100%
                        </button>
                        <div className="w-px h-4 bg-stroke-divider mx-1" />
                        <button
                            onClick={() => editor.chain().focus().deleteSelection().run()}
                            className="p-1.5 rounded hover:bg-status-error/10 text-status-error transition-colors"
                            title={t('editor.delete_image')}
                        >
                            <Trash2 className="w-4 h-4" />
                        </button>
                    </div>
                </BubbleMenu>
            )}

            {editor && textBubblePos && !editor.isActive('imageNodePro') && !(editor.state.selection as any).$anchorCell && (
                <div
                    style={{
                        position: 'fixed',
                        left: `${textBubblePos.x}px`,
                        top: `${textBubblePos.y - 10}px`,
                        transform: 'translate(-50%, -100%)',
                        zIndex: 100,
                    }}
                    onMouseDown={e => {
                        e.preventDefault();
                        e.stopPropagation();
                    }}
                >
                    <div className="flex items-center gap-0.5 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl p-1 animate-in fade-in zoom-in duration-200 z-[100] whitespace-nowrap">
                        <div className="flex items-center gap-0.5 flex-nowrap">
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

                                {showAiDropdown && (
                                    <div
                                        className={`absolute left-0 top-full mt-1 whitespace-normal bg-surface-flyout border border-stroke-divider rounded-lg shadow-xl p-1 z-[110] animate-in fade-in slide-in-from-top-1 duration-200 ${aiDropdownType === 'custom' ? 'w-72 max-h-none' : 'w-56 max-h-72 overflow-y-auto'}`}
                                        onMouseDown={e => {
                                            e.stopPropagation();
                                            if (aiDropdownType !== 'custom') {
                                                e.preventDefault();
                                            }
                                        }}
                                    >
                                        {aiDropdownType === 'main' && (
                                            <>
                                                {getEnabledEditorAiActions(settings?.editor?.aiActions).map((action: any) => (
                                                    <button
                                                        key={action.id}
                                                        onClick={() => handleAiImprove(action.prompt)}
                                                        className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left"
                                                    >
                                                        {(() => {
                                                            const Icon = ICON_MAP[action.icon] || Wand2;
                                                            return <Icon className="w-3.5 h-3.5 shrink-0" />;
                                                        })()}
                                                        {action.labelKey.includes('.') ? t(action.labelKey) : action.labelKey}
                                                    </button>
                                                ))}
                                                <div className="h-px bg-stroke-divider my-1 mx-1" />
                                                <button onClick={() => { setAiDropdownType('custom'); setCustomPrompt(''); }} className="w-full flex items-center gap-2.5 px-2.5 py-1.5 text-fs-xs font-medium text-text-secondary hover:bg-surface-subtle hover:text-text-primary rounded-md transition-colors text-left">
                                                    <Sparkles className="w-3.5 h-3.5 shrink-0" /> {t('editor.ai_custom')}
                                                </button>
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
                                                    <p className="mt-1.5 text-[10px] text-text-tertiary text-center">{t('editor.ctrl_enter_send')}</p>
                                                </div>
                                            </>
                                        )}
                                    </div>
                                )}
                            </div>

                            <div className="w-px h-4 bg-stroke-divider mx-1" />

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
                                                        : editor.isActive('paragraph', { variant: 'text2' }) ? t('turn_into.text2')
                                                            : editor.isActive('paragraph', { variant: 'text3' }) ? t('turn_into.text3')
                                                                : t('turn_into.text1')}
                                    </span>
                                    <ChevronDown className={`w-3 h-3 transition-transform ${showBubbleTurnInto ? 'rotate-180' : ''}`} />
                                </button>
                                {showBubbleTurnInto && (
                                    <div
                                        className="absolute left-0 top-full mt-1 w-44 bg-surface-flyout border border-stroke-divider rounded-lg shadow-xl p-1 z-[110] animate-in fade-in slide-in-from-top-1 duration-200"
                                        onMouseDown={e => e.stopPropagation()}
                                    >
                                        {([
                                            { key: 'text1', label: t('turn_into.text1'), icon: 'T1', action: () => editor.chain().focus().setParagraph().updateAttributes('paragraph', { variant: 'text1' }).run(), isActive: editor.isActive('paragraph', { variant: 'text1' }) },
                                            { key: 'text2', label: t('turn_into.text2'), icon: 'T2', action: () => editor.chain().focus().setParagraph().updateAttributes('paragraph', { variant: 'text2' }).run(), isActive: editor.isActive('paragraph', { variant: 'text2' }) },
                                            { key: 'text3', label: t('turn_into.text3'), icon: 'T3', action: () => editor.chain().focus().setParagraph().updateAttributes('paragraph', { variant: 'text3' }).run(), isActive: editor.isActive('paragraph', { variant: 'text3' }) },
                                            { key: 'h1', label: t('turn_into.heading1'), icon: 'H1', action: () => editor.chain().focus().setHeading({ level: 1 }).run(), isActive: editor.isActive('heading', { level: 1 }) },
                                            { key: 'h2', label: t('turn_into.heading2'), icon: 'H2', action: () => editor.chain().focus().setHeading({ level: 2 }).run(), isActive: editor.isActive('heading', { level: 2 }) },
                                            { key: 'h3', label: t('turn_into.heading3'), icon: 'H3', action: () => editor.chain().focus().setHeading({ level: 3 }).run(), isActive: editor.isActive('heading', { level: 3 }) },
                                            { key: 'h4', label: t('turn_into.heading4'), icon: 'H4', action: () => editor.chain().focus().setHeading({ level: 4 }).run(), isActive: editor.isActive('heading', { level: 4 }) },
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
                                className={`p-1.5 rounded transition-colors ${popupStates.bold ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.bold')}
                            >
                                <Bold className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().toggleItalic().run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.italic ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.italic')}
                            >
                                <Italic className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().toggleUnderline().run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.underline ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.underline')}
                            >
                                <UnderlineIcon className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().toggleHighlight().run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.highlight ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.highlight')}
                            >
                                <Highlighter className="w-4 h-4" />
                            </button>

                            <div className="w-px h-4 bg-stroke-divider mx-1" />

                            <button
                                onClick={() => editor.chain().focus().toggleBulletList().run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.bulletList ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('list_dropdown.bullet_list')}
                            >
                                <List className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().toggleOrderedList().run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.orderedList ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('list_dropdown.ordered_list')}
                            >
                                <ListOrdered className="w-4 h-4" />
                            </button>

                            <div className="w-px h-4 bg-stroke-divider mx-1" />
                            <button
                                onClick={() => editor.chain().focus().setTextAlign('left').run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.alignLeft ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.align_left')}
                            >
                                <AlignLeft className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().setTextAlign('center').run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.alignCenter ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.align_center')}
                            >
                                <AlignCenter className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().setTextAlign('right').run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.alignRight ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.align_right')}
                            >
                                <AlignRight className="w-4 h-4" />
                            </button>
                            <button
                                onClick={() => editor.chain().focus().setTextAlign('justify').run()}
                                className={`p-1.5 rounded transition-colors ${popupStates.alignJustify ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.justify')}
                            >
                                <AlignJustify className="w-4 h-4" />
                            </button>

                            <div className="w-px h-4 bg-stroke-divider mx-1" />
                            <button
                                onClick={() => {
                                    const url = window.prompt(t('editor.prompt_enter_url'));
                                    if (url) {
                                        editor.chain().focus().setLink({ href: url }).run();
                                    }
                                }}
                                className={`p-1.5 rounded transition-colors ${popupStates.link ? 'bg-accent-light2 text-accent-default' : 'text-text-secondary hover:bg-surface-subtle'}`}
                                title={t('editor.hyperlink')}
                            >
                                <LinkIcon className="w-4 h-4" />
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {editor && tableMenuPos && !textBubblePos && !editor.isActive('imageNodePro') && (
                <div
                    style={{
                        position: 'fixed',
                        left: `${tableMenuPos.x}px`,
                        top: `${tableMenuPos.y - 8}px`,
                        transform: 'translate(-50%, -100%)',
                        zIndex: 9999,
                    }}
                    onMouseDown={e => e.stopPropagation()}
                    className="flex items-center gap-0.5 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl px-2 py-1.5 animate-in fade-in zoom-in duration-200"
                >
                    {floatingTableActionGroups.map((group, groupIndex) => (
                        <React.Fragment key={group.key}>
                            {groupIndex > 0 && <div className="w-px h-4 bg-stroke-divider mx-1" />}
                            {group.actions.map((action) => {
                                const Icon = action.icon;
                                return (
                                    <button
                                        key={action.key}
                                        type="button"
                                        onClick={action.onClick}
                                        disabled={action.disabled}
                                        className={`p-1.5 rounded transition-colors ${action.danger
                                            ? 'text-status-error hover:bg-status-error/10'
                                            : 'text-text-secondary hover:bg-accent-light2 hover:text-accent-default'
                                            } ${action.disabled ? 'opacity-40 cursor-not-allowed hover:bg-transparent hover:text-text-secondary' : ''}`}
                                        title={action.label}
                                    >
                                        <Icon className="w-4 h-4" />
                                    </button>
                                );
                            })}
                        </React.Fragment>
                    ))}
                </div>
            )}


            <div
                className={`flex-1 overflow-y-auto relative transition-colors ${isDragOver ? 'ring-2 ring-inset ring-accent-default/50 bg-accent-default/5' : ''}`}
                data-editor-drop="true"
            >
                <EditorContent editor={editor} />
            </div>

            {(isAiImproving || aiImproveResult) && aiPanelAnchor && (
                <div ref={aiPanelRef} style={calcPanelStyle(aiPanelAnchor)}
                    className="bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl animate-in fade-in slide-in-from-bottom-2 duration-200">
                    <div className="flex flex-col">
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
                        {(aiImproveResult !== null && aiImproveResult !== '') && (
                            <div className="px-3 py-2.5 text-fs-sm text-text-primary leading-relaxed max-h-[200px] overflow-y-auto italic">
                                {aiImproveResult}
                                {isAiImproving && <span className="inline-block w-0.5 h-3.5 bg-accent-default animate-pulse ml-0.5 align-middle" />}
                            </div>
                        )}
                        {isAiImproving && (aiImproveResult === null || aiImproveResult === '') && (
                            <div className="px-3 py-3 flex items-center gap-2">
                                <div className="flex gap-1">
                                    <span className="w-1.5 h-1.5 bg-accent-default rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                                    <span className="w-1.5 h-1.5 bg-accent-default rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                                    <span className="w-1.5 h-1.5 bg-accent-default rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                                </div>
                            </div>
                        )}
                        {!isAiImproving && aiImproveResult && (
                            <div className="flex items-center gap-2 px-3 py-2 border-t border-stroke-divider">
                                <button onClick={() => {
                                    if (!editor || !aiImproveResult) return;
                                    const target = aiTargetRangeRef.current;
                                    if (target) {
                                        editor.chain().focus().insertContentAt({ from: target.from, to: target.to }, aiImproveResult).run();
                                    } else {
                                        editor.chain().focus().insertContent(aiImproveResult).run();
                                    }
                                    aiTargetRangeRef.current = null;
                                    setAiImproveResult(null);
                                    setAiPanelAnchor(null);
                                }}
                                    className="flex items-center gap-1 px-2.5 py-1.5 bg-accent-default text-white rounded-md text-[10px] font-medium hover:bg-accent-dark transition-colors">
                                    <Check className="w-3 h-3" /> {t('editor.ai_apply')}
                                </button>
                                <button onClick={() => {
                                    if (lastPromptRef.current) {
                                        setAiImproveResult(null);
                                        aiTargetRangeRef.current = selectedRangeRef.current;
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

            <div className="shrink-0 h-8 border-t border-stroke-divider px-4 flex items-center justify-between text-fs-xs text-text-tertiary bg-surface-subtle" onClick={(e) => e.stopPropagation()}>
                <span>{t('editor.words_count', { count: editor?.storage.characterCount?.words() || 0 })}</span>
                <div className="flex items-center gap-2">
                    <span>{t('editor.saved')}</span>
                    <div className="relative" ref={historyMenuRef}>
                        <button
                            type="button"
                            onClick={() => { setHistoryEntries(loadHistory(activeTabId)); setShowHistoryMenu(v => !v); }}
                            className={`p-1 rounded transition-colors ${showHistoryMenu ? 'bg-surface-base text-text-primary' : 'text-text-tertiary hover:bg-surface-base hover:text-text-secondary'}`}
                            title={t('editor.history_snapshots_title')}
                        >
                            <HistoryIcon className="w-3.5 h-3.5" />
                        </button>
                        {showHistoryMenu && (
                            <div className="absolute bottom-full right-0 mb-1 w-64 bg-surface-flyout border border-stroke-divider rounded-lg shadow-2xl z-[100] py-1.5 animate-in fade-in zoom-in duration-150">
                                <div className="px-3 py-1.5 text-[10px] font-medium text-text-tertiary border-b border-stroke-divider mb-1">
                                    {t('editor.history_snapshots_with_title', { title: tabs.find(t => t.id === activeTabId)?.title || '' })}
                                </div>
                                {historyEntries.length === 0 ? (
                                    <div className="px-3 py-3 text-fs-xs text-text-tertiary text-center">{t('editor.no_history_yet')}<br /><span className="text-[10px]">{t('editor.snapshot_every_5min')}</span></div>
                                ) : (
                                    <div className="max-h-60 overflow-y-auto">
                                        {historyEntries.map((entry, i) => (
                                            <button
                                                key={entry.ts}
                                                onClick={() => restoreHistory(entry)}
                                                className="w-full flex items-center justify-between px-3 py-2 text-fs-xs hover:bg-surface-subtle transition-colors text-left rounded-md"
                                            >
                                                <span className="text-text-secondary font-medium">{formatTs(entry.ts)}</span>
                                                <span className="text-text-tertiary text-[10px]">{t('editor.snapshot_number', { count: historyEntries.length - i })}</span>
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
