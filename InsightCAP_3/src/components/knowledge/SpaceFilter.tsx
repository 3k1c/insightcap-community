import React, { useEffect } from 'react';
import { Layers } from 'lucide-react';
import { useKnowledgeStore } from '../../stores/knowledgeStore';

export const SpaceFilter: React.FC = () => {
    const { spaces, activeSpaceId, setActiveSpaceId, loadSpaces } = useKnowledgeStore();

    useEffect(() => {
        loadSpaces();
    }, [loadSpaces]);

    return (
        <div className="flex flex-col h-full">
            <h2 className="text-xs font-semibold text-[var(--ic-text-secondary)] tracking-wider uppercase mb-4 px-2">
                分類 (Spaces)
            </h2>

            <div className="flex-1 overflow-y-auto space-y-1 pr-2">
                <button
                    onClick={() => setActiveSpaceId(null)}
                    className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors ${activeSpaceId === null
                            ? 'bg-[var(--ic-bg-active)] text-[var(--ic-text-primary)] font-medium shadow-sm border border-[var(--ic-border)]'
                            : 'text-[var(--ic-text-secondary)] hover:bg-[var(--ic-bg-subtle)] border border-transparent'
                        }`}
                >
                    <div className="flex items-center gap-2">
                        <Layers className="w-4 h-4" />
                        <span>全部知識</span>
                    </div>
                </button>

                {spaces.map(space => (
                    <button
                        key={space.id}
                        onClick={() => setActiveSpaceId(space.id)}
                        className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors ${activeSpaceId === space.id
                                ? 'bg-[var(--ic-bg-active)] text-[var(--ic-text-primary)] font-medium shadow-sm border border-[var(--ic-border)]'
                                : 'text-[var(--ic-text-secondary)] hover:bg-[var(--ic-bg-subtle)] border border-transparent'
                            }`}
                        title={space.description || space.name}
                    >
                        <span className="truncate">{space.name}</span>
                        <span className="text-xs text-[var(--ic-text-muted)] bg-[var(--ic-bg-subtle)] px-2 py-0.5 rounded-full border border-[var(--ic-border-muted)]">
                            {space.chunkCount}
                        </span>
                    </button>
                ))}

                {spaces.length === 0 && (
                    <div className="px-3 py-4 text-xs text-center text-[var(--ic-text-muted)]">
                        無分類資料，請擷取更多內容讓系統自動分群
                    </div>
                )}
            </div>
        </div>
    );
};
