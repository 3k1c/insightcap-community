export type EditorAiActionCategory = 'tone' | 'expand' | 'shorten' | 'translate' | 'custom';

export interface EditorAiAction {
    id: string;
    labelKey: string;
    icon: string;
    category: EditorAiActionCategory;
    prompt: string;
    enabled: boolean;
}

export const EDITOR_AI_ACTION_CATEGORIES: { value: EditorAiActionCategory; labelKey: string }[] = [
    { value: 'tone', labelKey: 'settings.ai_action_category_tone' },
    { value: 'expand', labelKey: 'settings.ai_action_category_expand' },
    { value: 'shorten', labelKey: 'settings.ai_action_category_shorten' },
    { value: 'translate', labelKey: 'settings.ai_action_category_translate' },
    { value: 'custom', labelKey: 'settings.ai_action_category_custom' },
];

export const DEFAULT_EDITOR_AI_ACTIONS: EditorAiAction[] = [
    {
        id: 'tone-business',
        labelKey: 'editor.ai_tone_business',
        icon: 'Briefcase',
        category: 'tone',
        prompt: 'Rewrite the selected text in a concise business tone. Preserve the original meaning and formatting where possible. Return only the rewritten text.',
        enabled: true,
    },
    {
        id: 'tone-friendly',
        labelKey: 'editor.ai_tone_friendly',
        icon: 'Smile',
        category: 'tone',
        prompt: 'Rewrite the selected text in a friendly and approachable tone. Preserve the original meaning and formatting where possible. Return only the rewritten text.',
        enabled: true,
    },
    {
        id: 'tone-formal',
        labelKey: 'editor.ai_tone_formal',
        icon: 'Shield',
        category: 'tone',
        prompt: 'Rewrite the selected text in a formal written tone. Preserve the original meaning and formatting where possible. Return only the rewritten text.',
        enabled: true,
    },
    {
        id: 'expand-moderate',
        labelKey: 'editor.ai_extend_moderate',
        icon: 'Maximize',
        category: 'expand',
        prompt: 'Expand the selected text with useful context and clearer transitions. Do not add unsupported facts. Return only the expanded text.',
        enabled: true,
    },
    {
        id: 'shorten-moderate',
        labelKey: 'editor.ai_shorten_moderate',
        icon: 'Minimize',
        category: 'shorten',
        prompt: 'Shorten the selected text while keeping the core meaning and important details. Return only the shortened text.',
        enabled: true,
    },
    {
        id: 'translate-zhtw',
        labelKey: 'editor.ai_translate_zhtw',
        icon: 'Languages',
        category: 'translate',
        prompt: 'Translate the selected text into Traditional Chinese. Keep technical terms in English where appropriate. Return only the translation.',
        enabled: true,
    },
    {
        id: 'translate-en',
        labelKey: 'editor.ai_translate_en',
        icon: 'Languages',
        category: 'translate',
        prompt: 'Translate the selected text into English. Preserve technical terms and formatting where appropriate. Return only the translation.',
        enabled: true,
    },
];

export function cloneDefaultEditorAiActions(): EditorAiAction[] {
    return DEFAULT_EDITOR_AI_ACTIONS.map(action => ({ ...action }));
}

export function normalizeEditorAiActions(actions?: Partial<EditorAiAction>[] | null): EditorAiAction[] {
    if (!Array.isArray(actions) || actions.length === 0) {
        return cloneDefaultEditorAiActions();
    }

    const validCategories = new Set(EDITOR_AI_ACTION_CATEGORIES.map(category => category.value));
    const normalized = actions
        .filter(action => action.id && validCategories.has(action.category as EditorAiActionCategory))
        .map(action => ({
            id: String(action.id),
            labelKey: String(action.labelKey ?? action.id),
            icon: String(action.icon ?? 'Wand2'),
            category: action.category as EditorAiActionCategory,
            prompt: String(action.prompt || ''),
            enabled: action.enabled !== false,
        }));

    return normalized.length > 0 ? normalized : cloneDefaultEditorAiActions();
}

export function getVisibleEditorAiActions(actions: Partial<EditorAiAction>[] | undefined | null, category: EditorAiActionCategory): EditorAiAction[] {
    return normalizeEditorAiActions(actions).filter(action => (
        action.enabled && action.category === category && action.prompt.trim().length > 0
    ));
}

export function getEnabledEditorAiActions(actions: Partial<EditorAiAction>[] | undefined | null): EditorAiAction[] {
    return normalizeEditorAiActions(actions).filter(action => (
        action.enabled && action.prompt.trim().length > 0
    ));
}

export function createEditorAiAction(category: EditorAiActionCategory = 'custom'): EditorAiAction {
    const id = `custom-${Date.now()}`;
    return {
        id,
        labelKey: '',
        icon: 'Sparkles',
        category,
        prompt: '',
        enabled: true,
    };
}

export function updateEditorAiAction(
    actions: Partial<EditorAiAction>[] | undefined | null,
    id: string,
    patch: Partial<EditorAiAction>,
): EditorAiAction[] {
    return normalizeEditorAiActions(actions).map(action => (
        action.id === id ? { ...action, ...patch } : action
    ));
}

export function removeEditorAiAction(actions: Partial<EditorAiAction>[] | undefined | null, id: string): EditorAiAction[] {
    const next = normalizeEditorAiActions(actions).filter(action => action.id !== id);
    return next.length > 0 ? next : cloneDefaultEditorAiActions();
}

export function moveEditorAiAction(
    actions: Partial<EditorAiAction>[] | undefined | null,
    id: string,
    direction: 'up' | 'down',
): EditorAiAction[] {
    const next = normalizeEditorAiActions(actions);
    const index = next.findIndex(action => action.id === id);
    if (index < 0) return next;

    const targetIndex = direction === 'up' ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= next.length) return next;

    const [item] = next.splice(index, 1);
    next.splice(targetIndex, 0, item);
    return next;
}

export function reorderEditorAiAction(
    actions: Partial<EditorAiAction>[] | undefined | null,
    draggedId: string,
    targetId: string,
): EditorAiAction[] {
    const next = normalizeEditorAiActions(actions);
    const draggedIndex = next.findIndex(action => action.id === draggedId);
    const targetIndex = next.findIndex(action => action.id === targetId);

    if (draggedIndex < 0 || targetIndex < 0 || draggedIndex === targetIndex) return next;

    const [item] = next.splice(draggedIndex, 1);
    next.splice(targetIndex, 0, item);
    return next;
}
