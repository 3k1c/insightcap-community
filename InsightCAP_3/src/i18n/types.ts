/**
 * i18n 型別定義 — 防止 key 拼錯
 * 所有介面文字須通過 t('key') 取值
 */

export interface TranslationKeys {
    common: {
        confirm: string;
        cancel: string;
        save: string;
        delete: string;
        edit: string;
        close: string;
        back: string;
        next: string;
        loading: string;
        error: string;
        success: string;
        warning: string;
        search: string;
        clear: string;
        copy: string;
        copied: string;
    };

    auth: {
        setup: {
            title: string;
            subtitle: string;
            step1_title: string;
            step2_title: string;
            step3_title: string;
            workspace_label: string;
            workspace_placeholder: string;
            workspace_hint: string;
            browse: string;
            password_label: string;
            password_placeholder: string;
            password_confirm_label: string;
            password_confirm_placeholder: string;
            password_mismatch: string;
            password_too_short: string;
            recovery_title: string;
            recovery_hint: string;
            recovery_confirm: string;
            finish: string;
        };
        login: {
            title: string;
            password_label: string;
            password_placeholder: string;
            submit: string;
            wrong_password: string;
            locked: string;
            permanently_locked: string;
            forgot_password: string;
        };
        recovery: {
            title: string;
            mnemonic_label: string;
            new_password_label: string;
            submit: string;
            invalid_mnemonic: string;
        };
    };

    theme: {
        label: string;
        light: string;
        dark: string;
        casual: string;
        fresh: string;
    };

    language: {
        label: string;
        'zh-TW': string;
        'zh-CN': string;
        en: string;
    };

    memory: {
        type: {
            data: string;
            pattern: string;
            log: string;
        };
    };

    settings: {
        title: string;
        general: string;
        appearance: string;
        ai_models: string;
        knowledge: string;
        hotkeys: string;
        security: string;
    };
}
