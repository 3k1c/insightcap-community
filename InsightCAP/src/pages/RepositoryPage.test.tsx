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
                return { todaySources: 0, totalChunks: 0, totalData: 0, totalPatterns: 0, totalLogs: 0 };
            }
            if (command === 'get_all_tags') return [];
            if (command === 'get_all_spaces') return [];
            if (command === 'get_source_ids_by_tag') return [];
            return undefined;
        });
    });

    it('shows the today timeline and import card when the repository has no sources or notes', async () => {
        render(<RepositoryPage />);

        expect(await screen.findByText('Import Files')).toBeInTheDocument();
        expect(screen.getByText('Choose local files')).toBeInTheDocument();
        expect(screen.getAllByText('Today').length).toBeGreaterThan(0);
        expect(screen.queryByText('No sources or notes yet')).not.toBeInTheDocument();
    });

    it('keeps the no-match empty state for an empty search result', async () => {
        const user = userEvent.setup();
        render(<RepositoryPage />);

        await user.type(screen.getByPlaceholderText('Search documents...'), 'missing');

        expect(await screen.findByText('No items match your search')).toBeInTheDocument();
        expect(screen.queryByText('Import Files')).not.toBeInTheDocument();
    });

    it('does not show the source import card while the Notes filter is active', async () => {
        const user = userEvent.setup();
        render(<RepositoryPage />);

        await user.click(await screen.findByRole('button', { name: 'Notes' }));

        expect(screen.getByText('No sources or notes yet')).toBeInTheDocument();
        expect(screen.queryByText('Import Files')).not.toBeInTheDocument();
    });
});
