/**
 * i18n — react-i18next 初始化
 * 語言：繁體中文（預設）/ 簡體中文 / 英文
 * 語言切換由 languageStore 控制，儲存於 localStorage
 */

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
    // localStorage 不可用
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
      escapeValue: false, // React 已做 XSS 防護
    },
  });

export default i18n;
