import { create } from 'zustand';

interface UiState {
    isSubSidebarCollapsed: boolean;
    toggleSubSidebar: (force?: boolean) => void;

    // Active conversation title (for TopBar display)
    activeConversationTitle: string;
    setActiveConversationTitle: (title: string) => void;

    // Active page navigation
    activePage: 'chat' | 'repository' | 'settings';
    setActivePage: (page: 'chat' | 'repository' | 'settings') => void;

    activeConversationId: string | null;
    setActiveConversationId: (id: string | null) => void;

    // Toggle editor pane in chat page
    isEditorOpen: boolean;
    toggleEditor: (force?: boolean) => void;

    // Conversation sidebar visibility
    isSidebarOpen: boolean;
    toggleSidebar: (force?: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
    isSubSidebarCollapsed: false,
    toggleSubSidebar: (force) => set((state) => ({
        isSubSidebarCollapsed: force !== undefined ? force : !state.isSubSidebarCollapsed
    })),

    activeConversationTitle: '',
    setActiveConversationTitle: (title) => set({ activeConversationTitle: title }),

    activePage: 'chat',
    setActivePage: (page) => set({ activePage: page }),

    activeConversationId: null,
    setActiveConversationId: (id) => set({ activeConversationId: id }),

    isEditorOpen: false,
    toggleEditor: (force) => set((state) => {
        const next = force !== undefined ? force : !state.isEditorOpen;
        // 開啟編輯器時自動收起側邊欄
        return { isEditorOpen: next, isSidebarOpen: next ? false : state.isSidebarOpen };
    }),

    isSidebarOpen: true,
    toggleSidebar: (force) => set((state) => ({
        isSidebarOpen: force !== undefined ? force : !state.isSidebarOpen
    })),
}));
