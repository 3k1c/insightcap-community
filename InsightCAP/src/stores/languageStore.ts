/**
 * languageStore — 語言切換狀態
 * localStorage 持久化
 */

import { create } from 'zustand';
import i18n, { LANGUAGE_STORAGE_KEY, type Language } from '../i18n';

interface LanguageState {
    language: Language;
    setLanguage: (language: Language) => void;
}

function readStoredLanguage(): Language {
    try {
        const stored = localStorage.getItem(LANGUAGE_STORAGE_KEY) as Language | null;
        if (stored && ['zh-TW', 'zh-CN', 'en'].includes(stored)) return stored;
    } catch {
        // ignore
    }
    return 'zh-TW';
}

const initialLanguage = readStoredLanguage();

export const useLanguageStore = create<LanguageState>(() => ({
    language: initialLanguage,

    setLanguage: (language: Language) => {
        i18n.changeLanguage(language);
        try {
            localStorage.setItem(LANGUAGE_STORAGE_KEY, language);
        } catch {
            // ignore
        }
        useLanguageStore.setState({ language });
    },
}));
