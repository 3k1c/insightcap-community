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

export interface TimelineSourceItem {
    id: string;
    title: string;
    type: string;
    sourceCategory: string; // 'editor_doc' | 'captured'
    mediaType: string;      // 'text' | 'url' | 'image' | 'video' | 'pdf'
    url?: string;
    filePath?: string;
    localDocPath?: string;
    thumbnail?: string;
    capturedAt: string;
    contentPreview: string;
    captureCount: number;
    tags: string[];
}

export interface CaptureItem {
    id: string;
    sourceId?: string;
    cleanContent: string;
    type: string;
    status: string;
    createdAt: string;
}

export interface CaptureDetail {
    id: string;
    sourceId?: string;
    spaceId?: string;
    cleanContent: string;
    type: string;
    tags: string;
    status: string;
    isUserEdited: boolean;
    createdAt: string;
    updatedAt: string;
}

export interface SpaceItem {
    id: string;
    name: string;
    description: string;
    chunkCount: number;
    isUserManaged: boolean;
}

export type CategoryFilter = 'all' | 'editor_doc' | 'captured';
export type MediaFilter = string | null; // null = all

export interface RepositoryStats {
    todaySources: number;
    totalChunks: number;
    totalData: number;
    totalPatterns: number;
    totalLogs: number;
}

export interface TimelineGroup {
    label: string;   // 'today' | 'yesterday' | 'this_week' | 'earlier' | date string
    items: TimelineSourceItem[];
}

interface KnowledgeState {
    sources: SourceItem[];
    pendingCaptures: CaptureItem[];
    spaces: SpaceItem[];
    activeSpaceId: string | null;
    isLoadingSources: boolean;
    isLoadingCaptures: boolean;

    timelineSources: TimelineSourceItem[];
    timelineGroups: TimelineGroup[];
    categoryFilter: CategoryFilter;
    mediaFilter: MediaFilter;
    searchQuery: string;
    spaceFilter: string | null;
    isLoadingTimeline: boolean;
    expandedSourceId: string | null;
    expandedCaptures: CaptureDetail[];
    isLoadingCapDetail: boolean;
    repoStats: RepositoryStats | null;

    setActiveSpaceId: (id: string | null) => void;
    loadSources: () => Promise<void>;
    loadPendingCaptures: () => Promise<void>;
    loadSpaces: () => Promise<void>;
    loadRepoStats: () => Promise<void>;

    setCategoryFilter: (f: CategoryFilter) => void;
    setMediaFilter: (f: MediaFilter) => void;
    setSearchQuery: (q: string) => void;
    setSpaceFilter: (id: string | null) => void;
    loadTimeline: () => Promise<void>;
    expandSource: (id: string | null) => Promise<void>;

    createEditorDocument: (title: string) => Promise<TimelineSourceItem | null>;
    deleteSource: (id: string) => Promise<void>;
    createManualCapture: (sourceId: string | null, content: string, tags?: string, spaceId?: string) => Promise<CaptureDetail | null>;
    updateCapture: (captureId: string, content?: string, tags?: string, spaceId?: string) => Promise<void>;
    deleteCapture: (captureId: string) => Promise<void>;
    createSpace: (name: string) => Promise<string | null>;
    deleteSpace: (spaceId: string) => Promise<void>;
}

function groupByDate(items: TimelineSourceItem[]): TimelineGroup[] {
    const now = new Date();
    const todayStr = now.toISOString().slice(0, 10);
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    const yesterdayStr = yesterday.toISOString().slice(0, 10);
    const weekAgo = new Date(now);
    weekAgo.setDate(weekAgo.getDate() - 7);

    const groups: Record<string, TimelineSourceItem[]> = {};
    const order: string[] = [];

    for (const item of items) {
        const d = item.capturedAt.slice(0, 10);
        let label: string;
        if (d === todayStr) label = 'today';
        else if (d === yesterdayStr) label = 'yesterday';
        else if (new Date(d) >= weekAgo) label = 'this_week';
        else label = 'earlier';

        if (!groups[label]) {
            groups[label] = [];
            order.push(label);
        }
        groups[label].push(item);
    }

    return order.map(label => ({ label, items: groups[label] }));
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
    sources: [],
    pendingCaptures: [],
    spaces: [],
    activeSpaceId: null,
    isLoadingSources: false,
    isLoadingCaptures: false,

    timelineSources: [],
    timelineGroups: [],
    categoryFilter: 'all',
    mediaFilter: null,
    searchQuery: '',
    spaceFilter: null,
    isLoadingTimeline: false,
    expandedSourceId: null,
    expandedCaptures: [],
    isLoadingCapDetail: false,
    repoStats: null,

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

    loadRepoStats: async () => {
        try {
            const stats = await invoke<RepositoryStats>('get_repository_stats');
            set({ repoStats: stats });
        } catch (error) {
            console.error('Failed to load repository stats:', error);
        }
    },


    setCategoryFilter: (f) => {
        set({ categoryFilter: f });
        get().loadTimeline();
    },

    setMediaFilter: (f) => {
        set({ mediaFilter: f });
        get().loadTimeline();
    },

    setSearchQuery: (q) => {
        set({ searchQuery: q });
        get().loadTimeline();
    },

    setSpaceFilter: (id) => {
        set({ spaceFilter: id });
        get().loadTimeline();
    },

    loadTimeline: async () => {
        set({ isLoadingTimeline: true });
        try {
            const { categoryFilter, mediaFilter, searchQuery, spaceFilter } = get();
            const items = await invoke<TimelineSourceItem[]>('get_sources_timeline', {
                category: categoryFilter === 'all' ? null : categoryFilter,
                mediaType: mediaFilter,
                searchQuery: searchQuery || null,
                spaceId: spaceFilter || null,
                limit: 100,
                offset: 0,
            });
            set({ timelineSources: items, timelineGroups: groupByDate(items) });
        } catch (error) {
            console.error('Failed to load timeline:', error);
        } finally {
            set({ isLoadingTimeline: false });
        }
    },

    expandSource: async (id) => {
        if (!id) {
            set({ expandedSourceId: null, expandedCaptures: [] });
            return;
        }
        set({ expandedSourceId: id, isLoadingCapDetail: true });
        try {
            const captures = await invoke<CaptureDetail[]>('get_captures_detail', { sourceId: id });
            set({ expandedCaptures: captures });
        } catch (error) {
            console.error('Failed to load capture details:', error);
            set({ expandedCaptures: [] });
        } finally {
            set({ isLoadingCapDetail: false });
        }
    },


    createEditorDocument: async (title) => {
        try {
            const doc = await invoke<TimelineSourceItem>('create_editor_document', { title });
            get().loadTimeline();
            return doc;
        } catch (error) {
            console.error('Failed to create editor document:', error);
            return null;
        }
    },

    deleteSource: async (id) => {
        try {
            await invoke('delete_source', { sourceId: id });
            set((s) => ({
                timelineSources: s.timelineSources.filter(i => i.id !== id),
                timelineGroups: groupByDate(s.timelineSources.filter(i => i.id !== id)),
                expandedSourceId: s.expandedSourceId === id ? null : s.expandedSourceId,
                expandedCaptures: s.expandedSourceId === id ? [] : s.expandedCaptures,
            }));
        } catch (error) {
            console.error('Failed to delete source:', error);
        }
    },

    createManualCapture: async (sourceId, content, tags, spaceId) => {
        try {
            const cap = await invoke<CaptureDetail>('create_manual_capture', {
                sourceId, content, tags: tags ?? null, spaceId: spaceId ?? null,
            });
            if (get().expandedSourceId === sourceId) {
                set((s) => ({ expandedCaptures: [...s.expandedCaptures, cap] }));
            }
            return cap;
        } catch (error) {
            console.error('Failed to create capture:', error);
            return null;
        }
    },

    updateCapture: async (captureId, content, tags, spaceId) => {
        try {
            await invoke('update_capture', {
                captureId,
                content: content ?? null,
                tags: tags ?? null,
                spaceId: spaceId ?? null,
            });
            set((s) => ({
                expandedCaptures: s.expandedCaptures.map(c =>
                    c.id === captureId
                        ? {
                            ...c,
                            cleanContent: content ?? c.cleanContent,
                            tags: tags ?? c.tags,
                            spaceId: spaceId ?? c.spaceId,
                            isUserEdited: true,
                        }
                        : c
                ),
            }));
        } catch (error) {
            console.error('Failed to update capture:', error);
        }
    },

    deleteCapture: async (captureId) => {
        try {
            await invoke('delete_capture', { captureId });
            set((s) => ({
                expandedCaptures: s.expandedCaptures.filter(c => c.id !== captureId),
            }));
        } catch (error) {
            console.error('Failed to delete capture:', error);
        }
    },

    createSpace: async (name) => {
        try {
            const id = await invoke<string>('create_manual_space', { name });
            await get().loadSpaces();
            return id;
        } catch (error) {
            console.error('Failed to create space:', error);
            return null;
        }
    },

    deleteSpace: async (spaceId) => {
        try {
            await invoke('delete_space', { spaceId });
            await get().loadSpaces();
            if (get().spaceFilter === spaceId) {
                get().setSpaceFilter(null);
            }
        } catch (error) {
            console.error('Failed to delete space:', error);
        }
    },
}));
