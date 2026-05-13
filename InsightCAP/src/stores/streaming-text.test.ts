import { describe, expect, it } from 'vitest';
import { nextStreamingText } from './streaming-text';

describe('nextStreamingText', () => {
    it('reveals a large target incrementally instead of all at once', () => {
        const target = 'A'.repeat(500);
        const next = nextStreamingText('', target);

        expect(next.length).toBeGreaterThan(0);
        expect(next.length).toBeLessThan(target.length);
    });

    it('limits large backlog bursts so delayed responses do not appear all at once', () => {
        const target = 'A'.repeat(500);
        const next = nextStreamingText('', target);

        expect(next.length).toBeLessThanOrEqual(2);
    });

    it('flushes the final tail without hanging on the last characters', () => {
        const target = 'Streaming response text';
        const current = 'Streaming response te';

        expect(nextStreamingText(current, target)).toBe(target);
    });

    it('eventually reaches the target text', () => {
        const target = 'Streaming response text';
        let current = '';

        for (let i = 0; i < 40; i += 1) {
            current = nextStreamingText(current, target);
        }

        expect(current).toBe(target);
    });
});
