import React, { useEffect } from 'react';
import { useKnowledgeStore } from '../stores/knowledgeStore';
import { SpaceFilter } from '../components/knowledge/SpaceFilter';
import { SourceCard } from '../components/knowledge/SourceCard';

export const KnowledgePage: React.FC = () => {
    const { sources, isLoadingSources, loadSources, loadPendingCaptures } = useKnowledgeStore();

    useEffect(() => {
        loadSources();
        loadPendingCaptures();
    }, [loadSources, loadPendingCaptures]);

    return (
        <div className="flex-1 flex flex-col h-full bg-[var(--ic-bg-base)]">
            <div className="h-16 border-b border-[var(--ic-border)] flex items-center px-6 shrink-0 bg-[var(--ic-bg-surface)]">
                <h1 className="text-lg font-semibold">我的記憶庫</h1>
            </div>

            <div className="flex-1 flex overflow-hidden">
                <div className="w-64 border-r border-[var(--ic-border)] bg-[var(--ic-bg-subtle)] p-4 flex flex-col">
                    <SpaceFilter />
                </div>

                <div className="flex-1 overflow-y-auto p-6">
                    {isLoadingSources ? (
                        <div className="text-center py-10 text-[var(--ic-text-muted)] mt-10">載入中...</div>
                    ) : sources.length > 0 ? (
                        <div className="flex flex-col gap-4 max-w-4xl mx-auto">
                            {sources.map(source => (
                                <SourceCard key={source.id} source={source} />
                            ))}
                        </div>
                    ) : (
                        <div className="text-center py-10 text-[var(--ic-text-muted)] mt-10">此空間尚無文件</div>
                    )}
                </div>
            </div>
        </div>
    );
};
