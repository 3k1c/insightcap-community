import { useState } from 'react';
import { Editor } from '@tiptap/react';
import { ChevronRight, Scissors, Copy, Clipboard, Trash2, Wand2, ChevronsDownUp, ChevronsUpDown, Plus } from 'lucide-react';
import { useT } from '../../hooks/useT';

export type AiAction =
    | { type: 'rewrite'; style: string; styleLabel: string }
    | { type: 'shorten' }
    | { type: 'expand'; length: 'short' | 'medium' | 'long'; lengthLabel: string };

interface EditorContextMenuProps {
    x: number;
    y: number;
    editor: Editor;
    inTable: boolean;
    selectedText: string;
    onClose: () => void;
    onAiAction: (action: AiAction) => void;
}

export function EditorContextMenu({ x, y, editor, inTable, selectedText, onClose, onAiAction }: EditorContextMenuProps) {
    const T = useT();
    const [showRewriteMenu, setShowRewriteMenu] = useState(false);
    const [showExpandMenu, setShowExpandMenu] = useState(false);
    const [showInsertMenu, setShowInsertMenu] = useState(false);
    const [showDeleteMenu, setShowDeleteMenu] = useState(false);
    const hasSelection = selectedText.trim().length > 0;

    const REWRITE_STYLES = [
        { label: T('editor.ctx.styleFormal'),      value: 'formal' },
        { label: T('editor.ctx.styleCasual'),       value: 'casual' },
        { label: T('editor.ctx.styleAcademic'),     value: 'academic' },
        { label: T('editor.ctx.styleJournalistic'), value: 'journalistic' },
        { label: T('editor.ctx.styleNarrative'),    value: 'narrative' },
        { label: T('editor.ctx.styleBullet'),       value: 'bullet' },
    ];

    const TABLE_INSERT_OPS = [
        { label: T('editor.ctx.addRowBefore'),    cmd: 'addRowBefore' },
        { label: T('editor.ctx.addRowAfter'),     cmd: 'addRowAfter' },
        { label: T('editor.ctx.addColBefore'),    cmd: 'addColumnBefore' },
        { label: T('editor.ctx.addColAfter'),     cmd: 'addColumnAfter' },
    ];

    const TABLE_DELETE_OPS = [
        { label: T('editor.ctx.deleteRow'),    cmd: 'deleteRow' },
        { label: T('editor.ctx.deleteColumn'), cmd: 'deleteColumn' },
        { label: T('editor.ctx.deleteTable'),  cmd: 'deleteTable' },
    ];

    // Adjust position so menu doesn't go off-screen
    const menuWidth = 180;
    const adjustedX = (x + menuWidth > window.innerWidth - 8) ? x - menuWidth : x;
    const adjustedY = Math.min(y, window.innerHeight - 320);

    const itemCls = (danger?: boolean, disabled?: boolean) =>
        `w-full flex items-center gap-2 px-3 py-1.5 text-xs font-medium rounded-lg transition-colors ${
            disabled
                ? 'text-ic-text-muted dark:text-gray-600 cursor-default'
                : danger
                    ? 'text-red-500 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 hover:text-red-600 cursor-pointer'
                    : 'text-ic-text-secondary dark:text-gray-300 hover:bg-ic-bg-hover dark:hover:bg-gray-700 hover:text-ic-text-primary dark:hover:text-white cursor-pointer'
        }`;

    const sep = (key: string | number) => (
        <div key={key} className="h-px bg-ic-border dark:bg-gray-700 my-0.5" />
    );

    const handleCut = () => {
        if (!hasSelection) return;
        navigator.clipboard.writeText(selectedText).then(() => {
            editor.chain().focus().deleteSelection().run();
        });
        onClose();
    };

    const handleCopy = () => {
        if (!hasSelection) return;
        navigator.clipboard.writeText(selectedText);
        onClose();
    };

    const handlePastePlain = async () => {
        try {
            const text = await navigator.clipboard.readText();
            editor.chain().focus().insertContent(text).run();
        } catch (_) { /* clipboard permission denied */ }
        onClose();
    };

    const handleSelectAll = () => {
        editor.chain().focus().selectAll().run();
        onClose();
    };

    const handleDelete = () => {
        if (!hasSelection) return;
        editor.chain().focus().deleteSelection().run();
        onClose();
    };

    return (
        <div
            onMouseDown={e => e.stopPropagation()}
            style={{ position: 'fixed', left: adjustedX, top: adjustedY, zIndex: 60 }}
            className="bg-ic-bg-elevated dark:bg-gray-800 border border-ic-border dark:border-gray-700 rounded-xl shadow-lg py-1 px-1 min-w-[168px] select-none"
        >
            {/* AI section (shown when text is selected) */}
            {hasSelection && (
                <>
                    {/* AI Rewrite (with submenu) */}
                    <div
                        className="relative"
                        onMouseEnter={() => setShowRewriteMenu(true)}
                        onMouseLeave={() => setShowRewriteMenu(false)}
                    >
                        <button className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20 transition-colors cursor-pointer">
                            <Wand2 className="w-3.5 h-3.5 shrink-0" />
                            <span className="flex-1 text-left">{T('editor.ctx.aiRewrite')}</span>
                            <ChevronRight className="w-3 h-3 opacity-60" />
                        </button>
                        {showRewriteMenu && (
                            <div
                                className="absolute left-full top-0 bg-ic-bg-elevated dark:bg-gray-800 border border-ic-border dark:border-gray-700 rounded-xl shadow-lg py-1 px-1 min-w-[108px]"
                                style={{ zIndex: 61 }}
                            >
                                {REWRITE_STYLES.map(s => (
                                    <button
                                        key={s.value}
                                        onClick={() => { onAiAction({ type: 'rewrite', style: s.value, styleLabel: s.label }); onClose(); }}
                                        className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary dark:text-gray-300 hover:text-ic-text-primary dark:hover:text-white hover:bg-ic-bg-hover dark:hover:bg-gray-700 rounded-lg transition-colors"
                                    >
                                        {s.label}
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    {/* AI Shorten */}
                    <button
                        onClick={() => { onAiAction({ type: 'shorten' }); onClose(); }}
                        className={itemCls()}
                    >
                        <ChevronsDownUp className="w-3.5 h-3.5 shrink-0 text-purple-500 dark:text-purple-400" />
                        <span>{T('editor.ctx.aiShorten')}</span>
                    </button>

                    {/* AI Expand (with submenu: short/medium/long) */}
                    <div
                        className="relative"
                        onMouseEnter={() => setShowExpandMenu(true)}
                        onMouseLeave={() => setShowExpandMenu(false)}
                    >
                        <button className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20 transition-colors cursor-pointer">
                            <ChevronsUpDown className="w-3.5 h-3.5 shrink-0" />
                            <span className="flex-1 text-left">{T('editor.ctx.aiExpand')}</span>
                            <ChevronRight className="w-3 h-3 opacity-60" />
                        </button>
                        {showExpandMenu && (
                            <div
                                className="absolute left-full top-0 bg-ic-bg-elevated dark:bg-gray-800 border border-ic-border dark:border-gray-700 rounded-lg shadow-xl py-1 min-w-[80px]"
                                style={{ zIndex: 61 }}
                            >
                                {([
                                    { label: T('editor.ctx.expandShort'),  value: 'short' },
                                    { label: T('editor.ctx.expandMedium'), value: 'medium' },
                                    { label: T('editor.ctx.expandLong'),   value: 'long' },
                                ] as const).map(opt => (
                                    <button
                                        key={opt.value}
                                        onClick={() => { onAiAction({ type: 'expand', length: opt.value, lengthLabel: opt.label }); onClose(); }}
                                        className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary dark:text-gray-300 hover:text-ic-text-primary dark:hover:text-white hover:bg-ic-bg-hover dark:hover:bg-gray-700 rounded-lg transition-colors"
                                    >
                                        {opt.label}
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    {sep('ai-sep')}
                </>
            )}

            {/* Standard edit actions */}
            <button onClick={handleCut} disabled={!hasSelection} className={itemCls(false, !hasSelection)}>
                <Scissors className="w-3.5 h-3.5 shrink-0" />
                <span>{T('editor.ctx.cut')}</span>
            </button>
            <button onClick={handleCopy} disabled={!hasSelection} className={itemCls(false, !hasSelection)}>
                <Copy className="w-3.5 h-3.5 shrink-0" />
                <span>{T('editor.ctx.copy')}</span>
            </button>
            <button onClick={handlePastePlain} className={itemCls()}>
                <Clipboard className="w-3.5 h-3.5 shrink-0" />
                <span>{T('editor.ctx.pastePlain')}</span>
            </button>
            <button onClick={handleSelectAll} className={itemCls()}>
                <span className="w-3.5 h-3.5 shrink-0" />
                <span>{T('editor.ctx.selectAll')}</span>
            </button>
            <button onClick={handleDelete} disabled={!hasSelection} className={itemCls(true, !hasSelection)}>
                <Trash2 className="w-3.5 h-3.5 shrink-0" />
                <span>{T('editor.ctx.delete')}</span>
            </button>

            {/* Table operations */}
            {inTable && (
                <>
                    {sep('table-sep')}

                    {/* Insert (with submenu) */}
                    <div
                        className="relative"
                        onMouseEnter={() => { setShowInsertMenu(true); setShowDeleteMenu(false); }}
                        onMouseLeave={() => setShowInsertMenu(false)}
                    >
                        <button className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-ic-text-secondary dark:text-gray-300 hover:bg-ic-bg-hover dark:hover:bg-gray-700 transition-colors cursor-pointer">
                            <Plus className="w-3.5 h-3.5 shrink-0" />
                            <span className="flex-1 text-left">{T('editor.ctx.insert')}</span>
                            <ChevronRight className="w-3 h-3 opacity-60" />
                        </button>
                        {showInsertMenu && (
                            <div
                                className="absolute left-full top-0 bg-ic-bg-elevated dark:bg-gray-800 border border-ic-border dark:border-gray-700 rounded-lg shadow-xl py-1 min-w-[140px]"
                                style={{ zIndex: 61 }}
                            >
                                {TABLE_INSERT_OPS.map(item => (
                                    <button
                                        key={item.cmd}
                                        onClick={() => { (editor.chain().focus() as any)[item.cmd]().run(); onClose(); }}
                                        className="w-full text-left px-3 py-1.5 text-xs font-medium text-ic-text-secondary dark:text-gray-300 hover:text-ic-text-primary dark:hover:text-white hover:bg-ic-bg-hover dark:hover:bg-gray-700 rounded-lg transition-colors"
                                    >
                                        {item.label}
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    {/* Delete (with submenu) */}
                    <div
                        className="relative"
                        onMouseEnter={() => { setShowDeleteMenu(true); setShowInsertMenu(false); }}
                        onMouseLeave={() => setShowDeleteMenu(false)}
                    >
                        <button className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-red-500 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors cursor-pointer">
                            <Trash2 className="w-3.5 h-3.5 shrink-0" />
                            <span className="flex-1 text-left">{T('editor.ctx.delete')}</span>
                            <ChevronRight className="w-3 h-3 opacity-60" />
                        </button>
                        {showDeleteMenu && (
                            <div
                                className="absolute left-full top-0 bg-ic-bg-elevated dark:bg-gray-800 border border-ic-border dark:border-gray-700 rounded-lg shadow-xl py-1 min-w-[120px]"
                                style={{ zIndex: 61 }}
                            >
                                {TABLE_DELETE_OPS.map(item => (
                                    <button
                                        key={item.cmd}
                                        onClick={() => { (editor.chain().focus() as any)[item.cmd]().run(); onClose(); }}
                                        className="w-full text-left px-3 py-1.5 text-xs font-medium text-red-500 dark:text-red-400 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                                    >
                                        {item.label}
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>
                </>
            )}
        </div>
    );
}
