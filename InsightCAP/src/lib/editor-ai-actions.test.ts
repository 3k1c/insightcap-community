import { describe, expect, it, vi } from 'vitest';
import {
    cloneDefaultEditorAiActions,
    createEditorAiAction,
    getEnabledEditorAiActions,
    getVisibleEditorAiActions,
    moveEditorAiAction,
    normalizeEditorAiActions,
    reorderEditorAiAction,
    removeEditorAiAction,
    updateEditorAiAction,
} from './editor-ai-actions';

describe('editor AI actions', () => {
    it('uses default actions when stored settings are missing', () => {
        const actions = normalizeEditorAiActions(undefined);

        expect(actions.length).toBeGreaterThan(0);
        expect(actions.some(action => action.category === 'tone')).toBe(true);
        expect(actions.some(action => action.category === 'translate')).toBe(true);
    });

    it('filters visible actions by category and enabled state', () => {
        const visible = getVisibleEditorAiActions([
            { id: 'a', labelKey: 'A', icon: 'Wand2', category: 'tone', prompt: 'A', enabled: true },
            { id: 'b', labelKey: 'B', icon: 'Wand2', category: 'tone', prompt: 'B', enabled: false },
            { id: 'c', labelKey: 'C', icon: 'Wand2', category: 'translate', prompt: 'C', enabled: true },
            { id: 'd', labelKey: 'D', icon: 'Wand2', category: 'tone', prompt: '', enabled: true },
        ], 'tone');

        expect(visible.map(action => action.id)).toEqual(['a']);
    });

    it('builds a flat enabled menu without category grouping', () => {
        const visible = getEnabledEditorAiActions([
            { id: 'a', labelKey: 'A', icon: 'Wand2', category: 'tone', prompt: 'A', enabled: true },
            { id: 'b', labelKey: 'B', icon: 'Wand2', category: 'translate', prompt: 'B', enabled: true },
            { id: 'c', labelKey: 'C', icon: 'Wand2', category: 'custom', prompt: '', enabled: true },
            { id: 'd', labelKey: 'D', icon: 'Wand2', category: 'shorten', prompt: 'D', enabled: false },
        ]);

        expect(visible.map(action => action.id)).toEqual(['a', 'b']);
    });

    it('normalizes legacy custom actions with missing optional fields', () => {
        const actions = normalizeEditorAiActions([
            { id: 'legacy', category: 'custom', prompt: 'Rewrite this' },
        ]);

        expect(actions).toEqual([
            {
                id: 'legacy',
                labelKey: 'legacy',
                icon: 'Wand2',
                category: 'custom',
                prompt: 'Rewrite this',
                enabled: true,
            },
        ]);
    });

    it('updates an action without mutating the original list', () => {
        const original = cloneDefaultEditorAiActions();
        const updated = updateEditorAiAction(original, original[0].id, { enabled: false, labelKey: 'New label' });

        expect(updated[0].enabled).toBe(false);
        expect(updated[0].labelKey).toBe('New label');
        expect(original[0].enabled).toBe(true);
    });

    it('removes and reorders actions', () => {
        const original = cloneDefaultEditorAiActions();
        const removed = removeEditorAiAction(original, original[0].id);
        expect(removed.some(action => action.id === original[0].id)).toBe(false);

        const moved = moveEditorAiAction(original, original[1].id, 'up');
        expect(moved[0].id).toBe(original[1].id);
        expect(moved[1].id).toBe(original[0].id);
    });

    it('reorders actions by dragged and target ids for drag and drop', () => {
        const original = cloneDefaultEditorAiActions();
        const moved = reorderEditorAiAction(original, original[2].id, original[0].id);

        expect(moved[0].id).toBe(original[2].id);
        expect(moved[1].id).toBe(original[0].id);
        expect(moved[2].id).toBe(original[1].id);
    });

    it('creates custom actions with stable defaults', () => {
        vi.spyOn(Date, 'now').mockReturnValue(123);

        expect(createEditorAiAction()).toEqual({
            id: 'custom-123',
            labelKey: '',
            icon: 'Sparkles',
            category: 'custom',
            prompt: '',
            enabled: true,
        });
    });
});
