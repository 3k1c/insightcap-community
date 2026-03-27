import { useEffect, useRef } from 'react';
import { Editor } from '@tiptap/react';
import { X, ChevronUp, ChevronDown, Replace } from 'lucide-react';
import type { SearchReplaceStorage } from './SearchReplaceExtension';
import { useT } from '../../hooks/useT';

interface SearchReplaceBarProps {
    editor: Editor;
    mode: 'find' | 'replace';
    searchTerm: string;
    replaceTerm: string;
    onSearchChange: (v: string) => void;
    onReplaceChange: (v: string) => void;
    onClose: () => void;
}

export function SearchReplaceBar({
    editor,
    mode,
    searchTerm,
    replaceTerm,
    onSearchChange,
    onReplaceChange,
    onClose,
}: SearchReplaceBarProps) {
    const T = useT();
    const searchInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
    }, []);

    // Sync search term into extension storage on change
    useEffect(() => {
        (editor.commands as any).setSearchTerm(searchTerm);
    }, [searchTerm, editor]);

    const storage = editor.storage.searchReplace as SearchReplaceStorage;
    const matchCount = storage?.results?.length ?? 0;
    const currentIndex = storage?.currentIndex ?? 0;

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            if (e.shiftKey) {
                (editor.commands as any).findPrev();
            } else {
                (editor.commands as any).findNext();
            }
        }
        if (e.key === 'Escape') {
            onClose();
        }
    };

    const handleReplaceNext = () => {
        (editor.commands as any).replaceNext(replaceTerm);
    };

    const handleReplaceAll = () => {
        (editor.commands as any).replaceAll(replaceTerm);
    };

    const matchLabel = searchTerm
        ? matchCount === 0
            ? T('editor.searchNotFound')
            : `${currentIndex + 1} / ${matchCount}`
        : '';

    return (
        <div className="no-print absolute top-0 right-0 z-40 m-2 bg-ic-bg-elevated border border-ic-border rounded-xl shadow-2xl px-3 py-2 flex flex-col gap-2 min-w-[300px]">
            {/* 尋找列 */}
            <div className="flex items-center gap-2">
                <input
                    ref={searchInputRef}
                    type="text"
                    value={searchTerm}
                    onChange={e => onSearchChange(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder={T('editor.searchPlaceholder')}
                    className="flex-1 text-xs px-2 py-1 rounded border border-ic-border-strong bg-transparent text-ic-text-secondary focus:outline-none focus:ring-1 focus:ring-ic-accent/50"
                />
                <span className="text-xs text-ic-text-muted w-12 text-center shrink-0">{matchLabel}</span>
                <button
                    onClick={() => (editor.commands as any).findPrev()}
                    disabled={matchCount === 0}
                    className="p-1 rounded hover:bg-ic-bg-hover disabled:opacity-30 transition-colors"
                    title={T('editor.findPrev')}
                >
                    <ChevronUp className="w-3.5 h-3.5 text-ic-text-muted" />
                </button>
                <button
                    onClick={() => (editor.commands as any).findNext()}
                    disabled={matchCount === 0}
                    className="p-1 rounded hover:bg-ic-bg-hover disabled:opacity-30 transition-colors"
                    title={T('editor.findNext')}
                >
                    <ChevronDown className="w-3.5 h-3.5 text-ic-text-muted" />
                </button>
                <button
                    onClick={onClose}
                    className="p-1 rounded hover:bg-ic-bg-hover transition-colors"
                    title={T('editor.findClose')}
                >
                    <X className="w-3.5 h-3.5 text-ic-text-muted" />
                </button>
            </div>

            {/* 取代列（只在 replace 模式顯示） */}
            {mode === 'replace' && (
                <div className="flex items-center gap-2">
                    <input
                        type="text"
                        value={replaceTerm}
                        onChange={e => onReplaceChange(e.target.value)}
                        onKeyDown={e => { if (e.key === 'Escape') onClose(); }}
                        placeholder={T('editor.replacePlaceholder')}
                        className="flex-1 text-xs px-2 py-1 rounded border border-ic-border-strong bg-transparent text-ic-text-secondary focus:outline-none focus:ring-1 focus:ring-ic-accent/50"
                    />
                    <button
                        onClick={handleReplaceNext}
                        disabled={matchCount === 0}
                        className="px-2 py-1 text-xs rounded bg-ic-accent hover:bg-ic-accent-hover disabled:opacity-30 text-ic-accent-fg transition-colors flex items-center gap-1 shrink-0"
                        title={T('editor.replaceCurrent')}
                    >
                        <Replace className="w-3 h-3" />
                        {T('editor.replace')}
                    </button>
                    <button
                        onClick={handleReplaceAll}
                        disabled={matchCount === 0}
                        className="px-2 py-1 text-xs rounded bg-orange-500 hover:bg-orange-600 disabled:opacity-30 text-white transition-colors shrink-0"
                        title={T('editor.replaceAll')}
                    >
                        {T('editor.replaceAllBtn')}
                    </button>
                </div>
            )}
        </div>
    );
}
