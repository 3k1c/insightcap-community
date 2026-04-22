import React, { useState } from 'react';
import { ChevronDown, ChevronUp } from 'lucide-react';
import type { ContextStats } from '../../stores/chatStore';
import { useT } from '../../hooks/useT';

interface ContextHintBannerProps {
    stats: ContextStats;
    isInjecting?: boolean;
}

export const ContextHintBanner: React.FC<ContextHintBannerProps> = ({
    stats,
    isInjecting = false
}) => {
    const t = useT();
    const total = stats.dataCount + stats.patternCount + stats.logCount;
    const [isExpanded, setIsExpanded] = useState(false);

    if (total === 0 && !isInjecting) {
        return null;
    }

    return (
        <div className="border-y border-stroke-divider bg-surface-subtle animate-in fade-in slide-in-from-top-2 duration-300">
            <div
                className="flex items-center justify-between px-3 py-2 text-fs-xs text-text-tertiary cursor-pointer select-none"
                onClick={() => !isInjecting && total > 0 && setIsExpanded(v => !v)}
            >
                <div className="flex items-center gap-2">
                    {isInjecting ? (
                        <span className="flex items-center gap-2 text-accent-default">
                            <span className="relative flex h-2 w-2">
                                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-current opacity-75"></span>
                                <span className="relative inline-flex rounded-full h-2 w-2 bg-current"></span>
                            </span>
                            {t('context_hint.injecting')}
                        </span>
                    ) : (
                        <span className="flex items-center gap-3">
                            {stats.patternCount > 0 && (
                                <span className="flex items-center gap-1">
                                    <span className="text-[var(--knowledge-pattern)]">*</span>
                                    {stats.patternHints.length > 0
                                        ? <span className="text-text-secondary">{stats.patternHints[0]}{stats.patternHints[0].length >= 50 ? '...' : ''}</span>
                                        : <span>{t('context_hint.pattern_short', { count: stats.patternCount })}</span>
                                    }
                                </span>
                            )}
                            {stats.logCount > 0 && (
                                <span className="flex items-center gap-1">
                                    <span className="text-[var(--knowledge-log)]">*</span>
                                    {stats.logHints.length > 0
                                        ? <span className="text-text-secondary">{stats.logHints[0]}{stats.logHints[0].length >= 50 ? '...' : ''}</span>
                                        : <span>{t('context_hint.log_short', { count: stats.logCount })}</span>
                                    }
                                </span>
                            )}
                            {stats.dataCount > 0 && (
                                <span className="flex items-center gap-1">
                                    <span className="text-[var(--knowledge-data)]">*</span>
                                    <span>{t('context_hint.data_short', { count: stats.dataCount })}</span>
                                </span>
                            )}
                        </span>
                    )}
                </div>

                {!isInjecting && total > 0 && (
                    isExpanded
                        ? <ChevronUp size={14} className="text-text-tertiary" />
                        : <ChevronDown size={14} className="text-text-tertiary" />
                )}
            </div>

            {isExpanded && !isInjecting && (
                <div className="px-3 pb-2 space-y-1.5">
                    {stats.patternHints.map((hint, i) => (
                        <div key={`p-${i}`} className="flex items-start gap-1.5 text-fs-xs">
                            <span className="text-[var(--knowledge-pattern)] mt-0.5">*</span>
                            <span className="text-text-secondary">{hint}{hint.length >= 50 ? '...' : ''}</span>
                        </div>
                    ))}
                    {stats.logHints.map((hint, i) => (
                        <div key={`l-${i}`} className="flex items-start gap-1.5 text-fs-xs">
                            <span className="text-[var(--knowledge-log)] mt-0.5">*</span>
                            <span className="text-text-secondary">{hint}{hint.length >= 50 ? '...' : ''}</span>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
};
