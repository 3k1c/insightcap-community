import { create } from 'zustand';
import { tauriCmd } from '../lib/tauri';

export interface EditorSelection {
    from: number;
    to: number;
    text: string;
}

interface UiState {
    isSubSidebarCollapsed: boolean;
    toggleSubSidebar: (force?: boolean) => void;

    // Active conversation title (for TopBar display)
    activeConversationTitle: string;
    setActiveConversationTitle: (title: string) => void;

    // Editor Panel
    isEditorOpen: boolean;
    activeEditorFilePath: string | null;
    openTabs: string[];
    openEditor: (filePath: string) => void;
    hideEditor: () => void;
    closeEditor: () => void;
    closeEditorTab: (filePath: string) => void;
    restoreLastOpenFile: () => Promise<void>;

    // 編輯器選取範圍（用於「覆蓋到編輯器」）
    pendingEditorSelection: EditorSelection | null;
    setPendingEditorSelection: (sel: EditorSelection | null) => void;

    // Active page navigation
    activePage: 'chat' | 'knowledge' | 'settings';
    setActivePage: (page: 'chat' | 'knowledge' | 'settings') => void;

    // Persist active conversation across page switches
    activeConversationId: string | null;
    setActiveConversationId: (id: string | null) => void;
}

export const useUiStore = create<UiState>((set, get) => ({
    isSubSidebarCollapsed: false,
    toggleSubSidebar: (force) => set((state) => ({
        isSubSidebarCollapsed: force !== undefined ? force : !state.isSubSidebarCollapsed
    })),

    activeConversationTitle: '',
    setActiveConversationTitle: (title) => set({ activeConversationTitle: title }),

    // Editor Panel
    isEditorOpen: false,
    activeEditorFilePath: null,
    openTabs: [],

    openEditor: (filePath) => {
        set((state) => {
            const tabs = state.openTabs.includes(filePath)
                ? state.openTabs
                : [...state.openTabs, filePath];
            return { isEditorOpen: true, activeEditorFilePath: filePath, openTabs: tabs };
        });
        tauriCmd.setLastOpenFile(filePath).catch(e =>
            console.error('[uiStore] setLastOpenFile failed:', e)
        );
    },

    // 隱藏編輯器面板（保留所有分頁和當前活躍檔案，下次重新打開時還原）
    hideEditor: () => {
        set({ isEditorOpen: false });
    },

    closeEditor: () => {
        set({ isEditorOpen: false, activeEditorFilePath: null, openTabs: [] });
        tauriCmd.setLastOpenFile(null).catch(e =>
            console.error('[uiStore] setLastOpenFile(null) failed:', e)
        );
    },

    closeEditorTab: (filePath) => {
        const state = get();
        const tabs = state.openTabs.filter(t => t !== filePath);
        if (tabs.length === 0) {
            set({ isEditorOpen: false, activeEditorFilePath: null, openTabs: [] });
            tauriCmd.setLastOpenFile(null).catch(() => {});
        } else {
            const newActive = state.activeEditorFilePath === filePath
                ? tabs[tabs.length - 1]
                : state.activeEditorFilePath;
            set({ openTabs: tabs, activeEditorFilePath: newActive });
            if (state.activeEditorFilePath === filePath && newActive) {
                tauriCmd.setLastOpenFile(newActive).catch(() => {});
            }
        }
    },

    pendingEditorSelection: null,
    setPendingEditorSelection: (sel) => set({ pendingEditorSelection: sel }),

    activePage: 'chat',
    setActivePage: (page) => set({ activePage: page }),

    activeConversationId: null,
    setActiveConversationId: (id) => set({ activeConversationId: id }),

    restoreLastOpenFile: async () => {
        // 啟動時預設 AI 對話模式，不自動打開文本編輯器
        try {
            await tauriCmd.getLastOpenFile();
        } catch (e) {
            console.error('[uiStore] restoreLastOpenFile failed:', e);
        }
    },
}));
