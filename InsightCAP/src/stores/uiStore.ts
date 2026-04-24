import { create } from 'zustand';

interface UiState {
    isSubSidebarCollapsed: boolean;
    toggleSubSidebar: (force?: boolean) => void;

    activeConversationTitle: string;
    setActiveConversationTitle: (title: string) => void;

    activePage: 'chat' | 'repository' | 'settings' | 'schedule';
    setActivePage: (page: 'chat' | 'repository' | 'settings' | 'schedule') => void;

    activeConversationId: string | null;
    setActiveConversationId: (id: string | null) => void;

    isEditorOpen: boolean;
    toggleEditor: (force?: boolean) => void;

    isSidebarOpen: boolean;
    toggleSidebar: (force?: boolean) => void;

    isChatHidden: boolean;
    toggleChatHidden: () => void;
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
        return { isEditorOpen: next, isSidebarOpen: next ? false : state.isSidebarOpen };
    }),

    isSidebarOpen: true,
    toggleSidebar: (force) => set((state) => ({
        isSidebarOpen: force !== undefined ? force : !state.isSidebarOpen
    })),

    isChatHidden: false,
    toggleChatHidden: () => set((state) => ({ isChatHidden: !state.isChatHidden })),
}));
