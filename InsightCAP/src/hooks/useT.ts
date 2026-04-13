import { useTranslation } from 'react-i18next';
import '../i18n';

export function useT() {
    const { t } = useTranslation();
    return t;
}
