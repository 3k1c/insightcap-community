import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import i18n from '../i18n';
import { useKnowledgeStore } from '../stores/knowledgeStore';
import { useTagStore } from '../stores/tagStore';
import { RepositoryPage } from './RepositoryPage';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
    convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openUrl: vi.fn(),
}));

vi.mock('sonner', () => ({
    toast: {
        success: vi.fn(),
        error: vi.fn(),
        loading: vi.fn(() => 'toast-id'),
    },
}));

vi.mock('../lib/noteStore', () => ({
    loadFiles: vi.fn(() => []),
    loadContent: vi.fn(() => ''),
    deleteFile: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);

function resetStores() {
    useKnowledgeStore.setState({
        timelineSources: [],
        timelineGroups: [],
        spaces: [],
        isLoadingTimeline: false,
        repoStats: null,
    });
    useTagStore.setState({
        recentTags: [],
        suggestions: [],
        isLoading: false,
    });
}

describe('RepositoryPage empty state', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
        vi.clearAllMocks();
        resetStores();
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'get_sources_timeline') return [];
            if (command === 'get_repository_stats') {
                return { todaySources: 0, totalChunks: 0, totalTags: 0, totalData: 0, totalPatterns: 0, totalLogs: 0 };
            }
            if (command === 'get_all_tags') return [];
            if (command === 'get_all_spaces') return [];
            if (command === 'get_source_ids_by_tag') return [];
            return undefined;
        });
    });

    it('shows the today timeline and import card when the repository has no sources or notes', async () => {
        render(<RepositoryPage />);

        expect(await screen.findByText('Add Source File')).toBeInTheDocument();
        expect(screen.getByText('Choose local files')).toBeInTheDocument();
        expect(screen.getAllByText('Today').length).toBeGreaterThan(0);
        expect(screen.queryByText('No sources or notes yet')).not.toBeInTheDocument();
    });

    it('keeps the no-match empty state for an empty search result', async () => {
        const user = userEvent.setup();
        render(<RepositoryPage />);

        await user.type(screen.getByPlaceholderText('Search documents...'), 'missing');

        expect(await screen.findByText('No items match your search')).toBeInTheDocument();
        expect(screen.queryByText('Add Source File')).not.toBeInTheDocument();
    });

    it('shows only the note add card while the Notes filter is active', async () => {
        const user = userEvent.setup();
        render(<RepositoryPage />);

        await user.click(await screen.findByRole('button', { name: 'Notes' }));

        expect(screen.queryByText('Add Source File')).not.toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'New Note' })).toBeInTheDocument();
    });

    it('shows total tag count from repository stats instead of loaded tag limit', async () => {
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'get_sources_timeline') return [];
            if (command === 'get_repository_stats') {
                return { todaySources: 0, totalChunks: 0, totalTags: 123, totalData: 0, totalPatterns: 0, totalLogs: 0 };
            }
            if (command === 'get_all_tags') {
                return Array.from({ length: 100 }, (_, i) => ({
                    id: `tag-${i}`,
                    name: `tag-${i}`,
                    source: 'ai',
                    useCount: 1,
                    recentCount: 1,
                }));
            }
            if (command === 'get_all_spaces') return [];
            if (command === 'get_source_ids_by_tag') return [];
            return undefined;
        });

        render(<RepositoryPage />);

        expect(await screen.findByText('123')).toBeInTheDocument();
    });

    it('shows space count after tags and can trigger classification refresh', async () => {
        const user = userEvent.setup();
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'get_sources_timeline') return [];
            if (command === 'get_repository_stats') {
                return { todaySources: 0, totalChunks: 0, totalTags: 4, totalData: 0, totalPatterns: 0, totalLogs: 0 };
            }
            if (command === 'get_all_tags') return [];
            if (command === 'get_all_spaces') {
                return [
                    { id: 'space-1', name: 'Research', description: '', chunkCount: 2, isUserManaged: false },
                    { id: 'space-2', name: 'Notes', description: '', chunkCount: 1, isUserManaged: true },
                ];
            }
            if (command === 'trigger_space_recluster') return 3;
            if (command === 'get_source_ids_by_tag') return [];
            return undefined;
        });

        render(<RepositoryPage />);

        expect(await screen.findByText('Spaces')).toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Refresh Classification' })).toBeInTheDocument();

        await user.click(screen.getByRole('button', { name: 'Refresh Classification' }));

        expect(mockInvoke).toHaveBeenCalledWith('trigger_space_recluster');
    });

    it('keeps source and note add buttons visible when repository already has items', async () => {
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'get_sources_timeline') {
                return [{
                    id: 'source-1',
                    title: 'Existing Source',
                    type: 'file',
                    sourceCategory: 'captured',
                    mediaType: 'text',
                    capturedAt: new Date().toISOString(),
                    contentPreview: 'Existing content',
                    captureCount: 1,
                    tags: [],
                }];
            }
            if (command === 'get_repository_stats') {
                return { todaySources: 1, totalChunks: 1, totalTags: 0, totalData: 0, totalPatterns: 0, totalLogs: 0 };
            }
            if (command === 'get_all_tags') return [];
            if (command === 'get_all_spaces') {
                return [{ id: 'space-1', name: 'Research', description: '', chunkCount: 1, isUserManaged: false }];
            }
            if (command === 'get_source_ids_by_tag') return [];
            return undefined;
        });

        render(<RepositoryPage />);

        expect(await screen.findByRole('button', { name: 'Add Source File' })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'New Note' })).toBeInTheDocument();
    });
});
