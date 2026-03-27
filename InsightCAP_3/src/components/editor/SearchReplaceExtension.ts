import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export interface SearchReplaceStorage {
    searchTerm: string;
    caseSensitive: boolean;
    results: Array<{ from: number; to: number }>;
    currentIndex: number;
}

const searchReplacePluginKey = new PluginKey<DecorationSet>('searchReplace');

function findMatches(
    doc: any,
    term: string,
    caseSensitive: boolean,
): Array<{ from: number; to: number }> {
    const results: Array<{ from: number; to: number }> = [];
    if (!term) return results;

    const needle = caseSensitive ? term : term.toLowerCase();

    doc.descendants((node: any, pos: number) => {
        if (!node.isText || !node.text) return;
        const haystack = caseSensitive ? node.text : node.text.toLowerCase();
        let idx = 0;
        while ((idx = haystack.indexOf(needle, idx)) !== -1) {
            results.push({ from: pos + idx, to: pos + idx + needle.length });
            idx += needle.length;
        }
    });

    return results;
}

export const SearchReplaceExtension = Extension.create<object, SearchReplaceStorage>({
    name: 'searchReplace',

    addStorage(): SearchReplaceStorage {
        return {
            searchTerm: '',
            caseSensitive: false,
            results: [],
            currentIndex: 0,
        };
    },

    addCommands() {
        return {
            setSearchTerm: (term: string) => ({ editor }: any) => {
                const storage = editor.storage.searchReplace as SearchReplaceStorage;
                storage.searchTerm = term;
                storage.currentIndex = 0;
                // Dispatch empty tr to trigger plugin re-calculation
                editor.view.dispatch(editor.view.state.tr.setMeta('searchReplace', true));
                return true;
            },

            findNext: () => ({ editor }: any) => {
                const storage = editor.storage.searchReplace as SearchReplaceStorage;
                if (!storage.results.length) return false;
                storage.currentIndex = (storage.currentIndex + 1) % storage.results.length;
                const { from, to } = storage.results[storage.currentIndex];
                editor.commands.setTextSelection({ from, to });
                editor.commands.scrollIntoView();
                editor.view.dispatch(editor.view.state.tr.setMeta('searchReplace', true));
                return true;
            },

            findPrev: () => ({ editor }: any) => {
                const storage = editor.storage.searchReplace as SearchReplaceStorage;
                if (!storage.results.length) return false;
                storage.currentIndex =
                    (storage.currentIndex - 1 + storage.results.length) % storage.results.length;
                const { from, to } = storage.results[storage.currentIndex];
                editor.commands.setTextSelection({ from, to });
                editor.commands.scrollIntoView();
                editor.view.dispatch(editor.view.state.tr.setMeta('searchReplace', true));
                return true;
            },

            replaceNext: (replaceTerm: string) => ({ editor }: any) => {
                const storage = editor.storage.searchReplace as SearchReplaceStorage;
                if (!storage.results.length) return false;
                const { from, to } = storage.results[storage.currentIndex];
                editor.chain().focus().setTextSelection({ from, to }).insertContent(replaceTerm).run();
                return true;
            },

            replaceAll: (replaceTerm: string) => ({ editor }: any) => {
                const storage = editor.storage.searchReplace as SearchReplaceStorage;
                if (!storage.results.length) return false;
                // Replace from end → start to preserve character positions
                const sorted = [...storage.results].sort((a, b) => b.from - a.from);
                editor.chain().focus().command(({ tr, dispatch }: any) => {
                    for (const { from, to } of sorted) {
                        tr.replaceWith(from, to, replaceTerm
                            ? editor.schema.text(replaceTerm)
                            : editor.schema.nodes.paragraph.create());
                    }
                    if (dispatch) dispatch(tr);
                    return true;
                }).run();
                return true;
            },
        } as any;
    },

    addProseMirrorPlugins() {
        const getStorage = () => this.storage as SearchReplaceStorage;

        return [
            new Plugin({
                key: searchReplacePluginKey,
                state: {
                    init: () => DecorationSet.empty,
                    apply(tr, _oldSet) {
                        const storage = getStorage();
                        if (!storage.searchTerm) {
                            storage.results = [];
                            return DecorationSet.empty;
                        }

                        const results = findMatches(tr.doc, storage.searchTerm, storage.caseSensitive);
                        storage.results = results;

                        const decorations = results.map((r, i) =>
                            Decoration.inline(r.from, r.to, {
                                class: i === storage.currentIndex
                                    ? 'search-match-current'
                                    : 'search-match',
                            })
                        );
                        return DecorationSet.create(tr.doc, decorations);
                    },
                },
                props: {
                    decorations(state) {
                        return searchReplacePluginKey.getState(state);
                    },
                },
            }),
        ];
    },
});
