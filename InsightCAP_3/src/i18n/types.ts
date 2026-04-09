/**
 * i18n 型別定義 — 防止 key 拼錯
 * 所有介面文字須通過 t('key') 取值
 */

export interface TranslationKeys {
    nav: {
        chat: string;
        repository: string;
        settings: string;
    };

    repository: {
        title: string;
        filter_all: string;
        filter_editor: string;
        filter_capture: string;
        media_text: string;
        media_url: string;
        media_image: string;
        media_video: string;
        media_pdf: string;
        today: string;
        yesterday: string;
        this_week: string;
        earlier: string;
        chunks: string;
        no_items: string;
        add_new: string;
        add_document: string;
        add_chunk: string;
        view_chunks: string;
        hide_chunks: string;
        edit_chunk: string;
        delete_chunk: string;
        delete_source: string;
        chunk_content: string;
        chunk_tags: string;
        chunk_space: string;
        confirm_delete: string;
        search_placeholder: string;
        doc_title_placeholder: string;
        stat_today_sources: string;
        stat_total_chunks: string;
        stat_total_data: string;
        stat_total_patterns: string;
        stat_total_logs: string;
        chunk_edit_space: string;
        chunk_edit_tags: string;
        chunk_save: string;
        chunk_cancel: string;
        chunk_tag_placeholder: string;
        chunk_save_success: string;
        chunk_no_space: string;
    };

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

    space_insight: {
        title: string;
        select_space: string;
        no_spaces: string;
        ready_label: string;
        caution_label: string;
        gap_label: string;
        pattern_count: string;
        log_count: string;
        data_count: string;
        capture_count: string;
        total_chunks: string;
        top_tags: string;
        no_chunks: string;
        collapse: string;
        expand: string;
    };

    context_hint: {
        injecting: string;
        pattern_short: string;
        log_short: string;
        data_short: string;
    };

    decision: {
        review_title: string;
        review_desc: string;
        variable: string;
        chosen: string;
        rating_good: string;
        rating_ok: string;
        rating_bad: string;
        rating_critical: string;
        note_placeholder: string;
        dismiss: string;
        submit: string;
        days_ago: string;
        no_pending: string;
    };

    pattern_promotion: {
        title: string;
        desc: string;
        confidence: string;
        remaining: string;
        accept: string;
        reject: string;
    };

    memory_confirm: {
        title: string;
        desc: string;
        confidence: string;
        remaining: string;
        accept: string;
        reject: string;
    };

    pending_drawer: {
        title: string;
        empty: string;
        select_all: string;
        deselect_all: string;
        accept_selected: string;
        reject_selected: string;
        confidence: string;
        type_data: string;
        type_pattern: string;
        type_log: string;
        close: string;
    };

    space_wiki: {
        tab_insight: string;
        tab_wiki: string;
        empty: string;
        edit: string;
        save: string;
        cancel: string;
        regenerate: string;
        regenerating: string;
        updated_at: string;
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

    turn_into: {
        label: string;
        text: string;
        heading1: string;
        heading2: string;
        heading3: string;
        heading4: string;
        heading5: string;
        heading6: string;
    };

    list_dropdown: {
        label: string;
        bullet_list: string;
        ordered_list: string;
    };

    editor: {
        export: string;
        export_txt: string;
        export_md: string;
        export_docx: string;
        export_pdf: string;
        ai_improve: string;
        ai_back: string;
        ai_fix_grammar: string;
        ai_extend: string;
        ai_shorten: string;
        ai_adjust_tone: string;
        ai_complete: string;
        ai_translate: string;
        ai_custom: string;
        ai_custom_label: string;
        ai_custom_placeholder: string;
        ai_custom_submit: string;
        ai_extend_slight: string;
        ai_extend_moderate: string;
        ai_extend_large: string;
        ai_shorten_slight: string;
        ai_shorten_moderate: string;
        ai_shorten_large: string;
        ai_tone_business: string;
        ai_tone_casual: string;
        ai_tone_confident: string;
        ai_tone_creative: string;
        ai_tone_emotional: string;
        ai_tone_excited: string;
        ai_tone_formal: string;
        ai_tone_friendly: string;
        ai_translate_zhtw: string;
        ai_translate_zhcn: string;
        ai_translate_en: string;
        ai_translate_ja: string;
        ai_improving: string;
        ai_result_title: string;
        ai_apply: string;
        ai_discard: string;
        ai_retry: string;
    };
}
