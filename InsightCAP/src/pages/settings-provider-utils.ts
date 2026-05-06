export type ModelCategory = 'embedding' | 'speech-to-text' | 'chat';

export interface ProviderOptionCard {
    value: string;
    label: string;
    defaultBaseUrl?: string;
}

export interface ProviderProfileOption {
    id: string;
    name: string;
    provider: string;
    baseUrl?: string;
    apiKey?: string;
}

export interface ModelOptionSettings {
    provider: string;
    model: string;
    apiKey?: string;
    baseUrl?: string;
}

export function buildProviderOptions(
    profiles: ProviderProfileOption[],
    cards: ProviderOptionCard[],
    includeLocal: boolean,
    localLabel: string,
) {
    const profiledProviders = new Set(profiles.map(p => p.provider));
    const options = [
        ...profiles.map(p => ({
            value: `profile:${p.id}`,
            label: p.name || p.provider,
        })),
        ...cards
            .filter(c => !profiledProviders.has(c.value))
            .map(c => ({
                value: `provider:${c.value}`,
                label: c.label,
            })),
    ];

    if (includeLocal) {
        options.push({ value: 'local', label: localLabel });
    }

    return options;
}

export function getSelectedProviderOptionValue(
    model: ModelOptionSettings,
    profiles: ProviderProfileOption[],
) {
    if (model.provider === 'local') {
        return 'local';
    }

    const matchingProfile = profiles.find(p => {
        if (p.provider !== model.provider) return false;
        const baseUrlMatches = !model.baseUrl || p.baseUrl === model.baseUrl;
        const apiKeyMatches = !model.apiKey || p.apiKey === model.apiKey;
        return baseUrlMatches && apiKeyMatches;
    });

    if (matchingProfile) {
        return `profile:${matchingProfile.id}`;
    }

    return `provider:${model.provider}`;
}

export function applyProviderOptionSelection(
    value: string,
    current: ModelOptionSettings,
    profiles: ProviderProfileOption[],
    cards: ProviderOptionCard[],
    category: ModelCategory,
    popularModels: Record<string, Array<{ value: string; category?: ModelCategory }>> = {},
): ModelOptionSettings {
    const selectedProfile = value.startsWith('profile:')
        ? profiles.find(p => p.id === value.slice('profile:'.length))
        : undefined;
    const provider = selectedProfile?.provider ?? value.replace(/^provider:/, '');
    const rawPopular = popularModels[provider] ?? [];
    const filtered = rawPopular.filter(m => {
        if (category === 'chat') return !m.category;
        return m.category === category;
    });
    const selectedCard = cards.find(c => c.value === provider);

    return {
        ...current,
        provider,
        model: filtered.length > 0 ? filtered[0].value : current.model,
        baseUrl: selectedProfile?.baseUrl ?? selectedCard?.defaultBaseUrl,
        apiKey: selectedProfile?.apiKey ?? '',
    };
}
