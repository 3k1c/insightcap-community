import React from 'react';
import { Database, Network, LayoutList } from 'lucide-react';

interface ContextStats {
    dataCount: number;
    patternCount: number;
    logCount: number;
}

interface ContextHintBannerProps {
    stats: ContextStats;
    isInjecting?: boolean;
}

export const ContextHintBanner: React.FC<ContextHintBannerProps> = ({
    stats,
    isInjecting = false
}) => {
    const total = stats.dataCount + stats.patternCount + stats.logCount;

    if (total === 0 && !isInjecting) {
        return null; /* 不顯示，以免干擾 */
    }

    return (
        <div className="flex items-center justify-between px-3 py-2 bg-[var(--ic-bg-subtle)] border-y border-[var(--ic-border)] text-xs text-[var(--ic-text-muted)] animate-in fade-in slide-in-from-top-2 duration-300">
            <div className="flex items-center gap-2">
                {isInjecting ? (
                    <span className="flex items-center gap-2 text-[var(--ic-accent-primary)]">
                        <span className="relative flex h-2 w-2">
                            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-current opacity-75"></span>
                            <span className="relative inline-flex rounded-full h-2 w-2 bg-current"></span>
                        </span>
                        檢索上下文擴充中...
                    </span>
                ) : (
                    <span>已注入 {total} 條相關脈絡：</span>
                )}
            </div>

            <div className="flex items-center gap-4">
                <div className="flex items-center gap-1.5" title="Data (原始資料/事實)">
                    <Database className="w-3.5 h-3.5 text-ic-memory-data" />
                    <span className="font-semibold">{stats.dataCount}</span>
                </div>

                <div className="flex items-center gap-1.5" title="Pattern (觀點/模式/模型)">
                    <Network className="w-3.5 h-3.5 text-ic-memory-pattern" />
                    <span className="font-semibold">{stats.patternCount}</span>
                </div>

                <div className="flex items-center gap-1.5" title="Log (行為日誌/操作)">
                    <LayoutList className="w-3.5 h-3.5 text-ic-memory-log" />
                    <span className="font-semibold">{stats.logCount}</span>
                </div>
            </div>
        </div>
    );
};
