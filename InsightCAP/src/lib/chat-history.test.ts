import { describe, expect, it } from 'vitest';
import { buildChatHistory } from './chat-history';

describe('buildChatHistory', () => {
    it('keeps only user and assistant messages', () => {
        const history = buildChatHistory([
            { role: 'system', content: 'internal note' },
            { role: 'user', content: 'hello' },
            { role: 'assistant', content: 'hi' },
        ]);

        expect(history).toEqual([
            ['user', 'hello'],
            ['assistant', 'hi'],
        ]);
    });

    it('keeps recent context even when older messages are long', () => {
        const history = buildChatHistory([
            { role: 'user', content: 'old ' + 'A'.repeat(5000) },
            { role: 'assistant', content: 'old answer ' + 'B'.repeat(5000) },
            { role: 'user', content: 'latest question' },
            { role: 'assistant', content: 'latest answer' },
        ], { maxChars: 1200, minRecentMessages: 2 });

        expect(history).toEqual([
            ['user', 'latest question'],
            ['assistant', 'latest answer'],
        ]);
    });

    it('adds older messages while staying inside the budget', () => {
        const history = buildChatHistory([
            { role: 'user', content: 'one' },
            { role: 'assistant', content: 'two' },
            { role: 'user', content: 'three' },
            { role: 'assistant', content: 'four' },
        ], { maxChars: 12, minRecentMessages: 2 });

        expect(history).toEqual([
            ['assistant', 'two'],
            ['user', 'three'],
            ['assistant', 'four'],
        ]);
    });
});
