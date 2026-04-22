
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import zhTW from './locales/zh-TW.json';
import zhCN from './locales/zh-CN.json';
import en from './locales/en.json';

export type Language = 'zh-TW' | 'zh-CN' | 'en';

export const LANGUAGE_STORAGE_KEY = 'ic-language';

function getStoredLanguage(): Language {
  try {
    const stored = localStorage.getItem(LANGUAGE_STORAGE_KEY) as Language | null;
    if (stored && ['zh-TW', 'zh-CN', 'en'].includes(stored)) return stored;
  } catch {
  }
  return 'zh-TW';
}

i18n
  .use(initReactI18next)
  .init({
    resources: {
      'zh-TW': { translation: zhTW },
      'zh-CN': { translation: zhCN },
      'en':    { translation: en },
    },
    lng: getStoredLanguage(),
    fallbackLng: 'zh-TW',
    interpolation: {
      escapeValue: false, // React already handles XSS escaping.
    },
  });

export default i18n;
