import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import i18n from '../i18n';
import { useKnowledgeStore } from '../stores/knowledgeStore';
import { useTagStore } from '../stores/tagStore';
import { RepositoryPage } from './RepositoryPage';

const eventHandlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
    convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
        eventHandlers.set(event, handler);
        return () => eventHandlers.delete(event);
    }),
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
const mockOpen = vi.mocked(open);

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
        eventHandlers.clear();
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

    it('hides add cards while the All filter is active', async () => {
        render(<RepositoryPage />);

        expect(await screen.findByText('No sources or notes yet')).toBeInTheDocument();
        expect(screen.queryByText('Add Source File')).not.toBeInTheDocument();
        expect(screen.queryByText('New Note')).not.toBeInTheDocument();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('shows only the source add card while the Sources filter is active', async () => {
        const user = userEvent.setup();
        render(<RepositoryPage />);

        await user.click(await screen.findByRole('button', { name: 'Sources' }));

        expect(screen.getByRole('button', { name: 'Add Source File' })).toBeInTheDocument();
        expect(screen.getByText('Choose local files')).toBeInTheDocument();
        expect(screen.queryByText('New Note')).not.toBeInTheDocument();
    });

    it('shows import progress after choosing source files', async () => {
        const user = userEvent.setup();
        mockOpen.mockResolvedValue(['C:\\docs\\alpha.pdf', 'C:\\docs\\beta.pdf']);
        render(<RepositoryPage />);

        await user.click(await screen.findByRole('button', { name: 'Sources' }));
        await user.click(screen.getByRole('button', { name: 'Add Source File' }));

        expect(screen.getByText('Processing progress')).toBeInTheDocument();
        expect(screen.getByText(/\/2$/)).toBeInTheDocument();
        expect(screen.getByText('alpha.pdf')).toBeInTheDocument();
        expect(screen.getByText('beta.pdf')).toBeInTheDocument();
    });

    it('shows background processing progress and auto closes after completion', () => {
        render(<RepositoryPage />);

        const handler = eventHandlers.get('processing-task-progress');
        expect(handler).toBeDefined();
        vi.useFakeTimers();

        act(() => {
            handler?.({
                payload: {
                    taskId: 'capture-1',
                    filePath: 'https://example.com/article',
                    fileName: 'Example Article',
                    stage: 'cleaning',
                    status: 'processing',
                    current: 0,
                    total: 1,
                    message: 'Cleaning',
                },
            });
        });

        expect(screen.getByText('Processing progress')).toBeInTheDocument();
        expect(screen.getByText('Example Article')).toBeInTheDocument();

        act(() => {
            handler?.({
                payload: {
                    taskId: 'capture-1',
                    filePath: 'https://example.com/article',
                    fileName: 'Example Article',
                    stage: 'completed',
                    status: 'done',
                    current: 1,
                    total: 1,
                    message: 'Done',
                },
            });
        });

        expect(screen.getByText('1/1')).toBeInTheDocument();

        act(() => {
            vi.advanceTimersByTime(4500);
        });

        expect(screen.queryByText('Processing progress')).not.toBeInTheDocument();
    });

    it('can cancel a processing task from the progress panel', async () => {
        const user = userEvent.setup();
        render(<RepositoryPage />);

        const handler = eventHandlers.get('processing-task-progress');
        expect(handler).toBeDefined();

        act(() => {
            handler?.({
                payload: {
                    taskId: 'capture-cancel-1',
                    filePath: 'C:\\docs\\slow.pdf',
                    fileName: 'slow.pdf',
                    stage: 'indexing',
                    status: 'processing',
                    current: 1,
                    total: 5,
                    message: 'Indexing',
                },
            });
        });

        await user.click(screen.getByRole('button', { name: 'Cancel slow.pdf' }));

        expect(mockInvoke).toHaveBeenCalledWith('cancel_processing_task', { taskId: 'capture-cancel-1' });
        expect(screen.getByText('Cancelled')).toBeInTheDocument();
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

    it('keeps add cards hidden while the All filter is active when repository already has items', async () => {
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

        expect(await screen.findByText('Existing Source')).toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'Add Source File' })).not.toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'New Note' })).not.toBeInTheDocument();
    });
});
