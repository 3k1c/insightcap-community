import { describe, expect, it } from 'vitest';
import {
    applyProviderOptionSelection,
    buildProviderOptions,
    getSelectedProviderOptionValue,
} from './settings-provider-utils';

const cards = [
    { value: 'ollama', label: 'Ollama', defaultBaseUrl: 'http://localhost:11434' },
    { value: 'openai', label: 'OpenAI' },
];

const profiles = [
    {
        id: 'desktop-ollama',
        name: 'Ollama Desktop',
        provider: 'ollama',
        baseUrl: 'http://192.168.1.10:11434',
        apiKey: '',
    },
    {
        id: 'laptop-ollama',
        name: 'Ollama Laptop',
        provider: 'ollama',
        baseUrl: 'http://192.168.1.20:11434',
        apiKey: '',
    },
];

describe('settings provider profile helpers', () => {
    it('keeps multiple profiles for the same provider selectable', () => {
        const options = buildProviderOptions(profiles, cards, false, 'Local');

        expect(options).toContainEqual({ value: 'profile:desktop-ollama', label: 'Ollama Desktop' });
        expect(options).toContainEqual({ value: 'profile:laptop-ollama', label: 'Ollama Laptop' });
        expect(options).not.toContainEqual({ value: 'provider:ollama', label: 'Ollama' });
    });

    it('applies the selected profile connection details to the model settings', () => {
        const current = { provider: 'ollama', model: 'qwen2.5:7b', baseUrl: 'http://localhost:11434', apiKey: '' };

        const next = applyProviderOptionSelection('profile:laptop-ollama', current, profiles, cards, 'chat');

        expect(next).toMatchObject({
            provider: 'ollama',
            model: 'qwen2.5:7b',
            baseUrl: 'http://192.168.1.20:11434',
            apiKey: '',
        });
    });

    it('selects the matching profile value from model baseUrl', () => {
        const model = { provider: 'ollama', model: 'qwen2.5:7b', baseUrl: 'http://192.168.1.10:11434' };

        expect(getSelectedProviderOptionValue(model, profiles)).toBe('profile:desktop-ollama');
    });
});
