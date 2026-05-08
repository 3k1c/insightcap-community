import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MessageList } from './MessageList';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openUrl: vi.fn(),
}));

describe('MessageList scope chips', () => {
    it('shows selected tag scopes on user messages', () => {
        render(
            <MessageList
                messages={[
                    {
                        id: 'msg-1',
                        role: 'user',
                        content: 'Summarize this',
                        created_at: '2026-05-06T00:00:00.000Z',
                        mentionedTags: [{ id: 'tag-pdf', name: 'PDF' }],
                    } as any,
                ]}
            />,
        );

        expect(screen.getByText('PDF')).toBeInTheDocument();
    });
});
