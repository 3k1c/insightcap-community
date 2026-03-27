import i18n from '../i18n';

export function useT() {
    return (key: string, vars?: Record<string, string>) => {
        if (i18n && typeof (i18n as any).t === 'function') {
            return (i18n as any).t(key, vars);
        }
        return key;
    };
}
