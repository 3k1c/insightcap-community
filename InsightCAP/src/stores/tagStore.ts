import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface Tag {
    id: string;
    name: string;
    source: string;
    useCount: number;
    recentCount: number;
}

interface TagState {
    recentTags: Tag[];
    suggestions: string[];
    isLoading: boolean;
    loadRecentTags: () => Promise<void>;
    fetchSuggestions: (query: string) => Promise<void>;
}

export const useTagStore = create<TagState>((set) => ({
    recentTags: [],
    suggestions: [],
    isLoading: false,

    loadRecentTags: async () => {
        set({ isLoading: true });
        try {
            const tags = await invoke<Tag[]>('get_all_tags');
            set({ recentTags: tags });
        } catch (error) {
            console.error('Failed to load tags:', error);
        } finally {
            set({ isLoading: false });
        }
    },

    fetchSuggestions: async (query: string) => {
        if (!query) {
            set({ suggestions: [] });
            return;
        }
        try {
            const suggestions = await invoke<string[]>('suggest_tags', { query });
            set({ suggestions });
        } catch (error) {
            console.error('Failed to fetch suggestions:', error);
        }
    },
}));
