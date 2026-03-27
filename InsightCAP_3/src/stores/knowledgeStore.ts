import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface SourceItem {
    id: string;
    title: string;
    type: string;
    url?: string;
    filePath?: string;
    capturedAt: string;
    contentPreview: string;
}

export interface CaptureItem {
    id: string;
    sourceId?: string;
    cleanContent: string;
    type: string;
    status: string;
    createdAt: string;
}

export interface SpaceItem {
    id: string;
    name: string;
    description: string;
    chunkCount: number;
}

interface KnowledgeState {
    sources: SourceItem[];
    pendingCaptures: CaptureItem[];
    spaces: SpaceItem[];
    activeSpaceId: string | null;
    isLoadingSources: boolean;
    isLoadingCaptures: boolean;

    setActiveSpaceId: (id: string | null) => void;
    loadSources: () => Promise<void>;
    loadPendingCaptures: () => Promise<void>;
    loadSpaces: () => Promise<void>;
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
    sources: [],
    pendingCaptures: [],
    spaces: [],
    activeSpaceId: null,
    isLoadingSources: false,
    isLoadingCaptures: false,

    setActiveSpaceId: (id) => {
        set({ activeSpaceId: id });
        get().loadSources();
    },

    loadSources: async () => {
        set({ isLoadingSources: true });
        try {
            const { activeSpaceId } = get();
            const sources = await invoke<SourceItem[]>('get_sources', {
                limit: 50,
                spaceId: activeSpaceId
            });
            set({ sources });
        } catch (error) {
            console.error('Failed to load sources:', error);
        } finally {
            set({ isLoadingSources: false });
        }
    },

    loadPendingCaptures: async () => {
        set({ isLoadingCaptures: true });
        try {
            const captures = await invoke<CaptureItem[]>('get_captures', { status: 'inbox', limit: 100 });
            set({ pendingCaptures: captures });
        } catch (error) {
            console.error('Failed to load pending captures:', error);
        } finally {
            set({ isLoadingCaptures: false });
        }
    },

    loadSpaces: async () => {
        try {
            const spaces = await invoke<SpaceItem[]>('get_all_spaces');
            set({ spaces });
        } catch (error) {
            console.error('Failed to load spaces:', error);
        }
    },
}));
