import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Layers, ChevronDown, ChevronUp, RefreshCw } from 'lucide-react';
import { useKnowledgeStore, type SpaceItem } from '../../stores/knowledgeStore';
import { useT } from '../../hooks/useT';


interface TagStat {
    tag: string;
    count: number;
}

interface SpaceInsight {
    spaceId: string;
    spaceName: string;
    totalChunks: number;
    dataCount: number;
    patternCount: number;
    logCount: number;
    captureCount: number;
    topTags: TagStat[];
}

interface SpaceKnowledgeGuide {
    spaceId: string;
    knowledgeGuideContent: string;
    knowledgeGuideUpdatedAt: string;
}

type ActiveTab = 'insight' | 'knowledge_guide';


export function SpaceInsightPanel() {
    const t = useT();
    const { spaces, loadSpaces } = useKnowledgeStore();
    const [selectedSpaceId, setSelectedSpaceId] = useState<string | null>(null);
    const [activeTab, setActiveTab] = useState<ActiveTab>('insight');
    const [insight, setInsight] = useState<SpaceInsight | null>(null);
    const [knowledgeGuide, setKnowledgeGuide] = useState<SpaceKnowledgeGuide | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const [isExpanded, setIsExpanded] = useState(true);

    useEffect(() => { loadSpaces(); }, [loadSpaces]);

    useEffect(() => {
        if (!selectedSpaceId || activeTab !== 'insight') { setInsight(null); return; }
        let cancelled = false;
        setIsLoading(true);
        invoke<SpaceInsight>('get_space_insight', { spaceId: selectedSpaceId })
            .then((data) => { if (!cancelled) setInsight(data); })
            .catch(() => { if (!cancelled) setInsight(null); })
            .finally(() => { if (!cancelled) setIsLoading(false); });
        return () => { cancelled = true; };
    }, [selectedSpaceId, activeTab]);

    useEffect(() => {
        if (!selectedSpaceId || activeTab !== 'knowledge_guide') { setKnowledgeGuide(null); return; }
        let cancelled = false;
        setIsLoading(true);
        invoke<SpaceKnowledgeGuide>('get_space_knowledge_guide', { spaceId: selectedSpaceId })
            .then((data) => { if (!cancelled) setKnowledgeGuide(data); })
            .catch(() => { if (!cancelled) setKnowledgeGuide(null); })
            .finally(() => { if (!cancelled) setIsLoading(false); });
        return () => { cancelled = true; };
    }, [selectedSpaceId, activeTab]);

    if (spaces.length === 0) return null;

    return (
        <div className="rounded-lg border border-stroke-divider bg-surface-card">
            <div
                className="flex items-center justify-between px-4 py-3 cursor-pointer select-none"
                onClick={() => setIsExpanded((v) => !v)}
            >
                <div className="flex items-center gap-2 text-fs-sm font-medium text-text-primary">
                    <Layers size={16} className="text-accent-default" />
                    {t('space_insight.title')}
                </div>
                {isExpanded
                    ? <ChevronUp size={16} className="text-text-tertiary" />
                    : <ChevronDown size={16} className="text-text-tertiary" />
                }
            </div>

            {isExpanded && (
                <div className="px-4 pb-4 space-y-3">
                    <select
                        className="w-full rounded-md border border-stroke-control bg-surface-layer px-3 py-1.5 text-fs-sm text-text-primary focus:outline-none focus:border-accent-default"
                        value={selectedSpaceId ?? ''}
                        onChange={(e) => {
                            setSelectedSpaceId(e.target.value || null);
                            setActiveTab('insight');
                        }}
                    >
                        <option value="">{t('space_insight.select_space')}</option>
                        {spaces.map((s: SpaceItem) => (
                            <option key={s.id} value={s.id}>
                                {s.name} ({s.chunkCount})
                            </option>
                        ))}
                    </select>

                    {selectedSpaceId && (
                        <div className="flex gap-1 rounded-md bg-surface-subtle p-0.5">
                            {(['insight', 'knowledge_guide'] as ActiveTab[]).map((tab) => (
                                <button
                                    key={tab}
                                    onClick={() => setActiveTab(tab)}
                                    className={`flex-1 rounded py-1 text-fs-xs font-medium transition-colors ${activeTab === tab
                                            ? 'bg-surface-card text-text-primary shadow-sm'
                                            : 'text-text-secondary hover:text-text-primary'
                                        }`}
                                >
                                    {tab === 'insight' ? t('space_knowledge_guide.tab_insight') : t('space_knowledge_guide.tab_guide')}
                                </button>
                            ))}
                        </div>
                    )}

                    {isLoading && (
                        <div className="text-fs-sm text-text-tertiary py-2">{t('common.loading')}</div>
                    )}

                    {!isLoading && activeTab === 'insight' && insight && insight.totalChunks === 0 && (
                        <div className="text-fs-sm text-text-tertiary py-2">{t('space_insight.no_chunks')}</div>
                    )}
                    {!isLoading && activeTab === 'insight' && insight && insight.totalChunks > 0 && (
                        <InsightContent insight={insight} t={t} />
                    )}

                    {!isLoading && activeTab === 'knowledge_guide' && selectedSpaceId && (
                        <KnowledgeGuideContent
                            spaceId={selectedSpaceId}
                            knowledgeGuide={knowledgeGuide}
                            onUpdated={setKnowledgeGuide}
                            t={t}
                        />
                    )}
                </div>
            )}
        </div>
    );
}


function InsightContent({ insight, t }: { insight: SpaceInsight; t: (key: string, opts?: Record<string, unknown>) => string }) {
    return (
        <div className="space-y-3">
            <div className="text-fs-xs text-text-tertiary">
                {t('space_insight.total_chunks', { count: insight.totalChunks })}
            </div>

            {insight.patternCount > 0 && (
                <div>
                    <div className="text-fs-xs font-medium text-text-secondary mb-1">{t('space_insight.ready_label')}</div>
                    <div className="flex items-center gap-1.5 text-fs-sm">
                        <span className="text-[var(--knowledge-pattern)]">*</span>
                        <span className="text-text-primary">{t('space_insight.pattern_count', { count: insight.patternCount })}</span>
                    </div>
                </div>
            )}

            {insight.logCount > 0 && (
                <div>
                    <div className="text-fs-xs font-medium text-text-secondary mb-1">{t('space_insight.caution_label')}</div>
                    <div className="flex items-center gap-1.5 text-fs-sm">
                        <span className="text-[var(--knowledge-log)]">*</span>
                        <span className="text-text-primary">{t('space_insight.log_count', { count: insight.logCount })}</span>
                    </div>
                </div>
            )}

            {(insight.dataCount > 0 || insight.captureCount > 0) && (
                <div className="flex flex-wrap gap-3 text-fs-sm">
                    {insight.dataCount > 0 && (
                        <div className="flex items-center gap-1.5">
                            <span className="text-[var(--knowledge-data)]">*</span>
                            <span className="text-text-primary">{t('space_insight.data_count', { count: insight.dataCount })}</span>
                        </div>
                    )}
                    {insight.captureCount > 0 && (
                        <div className="text-text-secondary">{t('space_insight.capture_count', { count: insight.captureCount })}</div>
                    )}
                </div>
            )}

            {insight.topTags.length > 0 && (
                <div>
                    <div className="text-fs-xs font-medium text-text-secondary mb-1.5">{t('space_insight.top_tags')}</div>
                    <div className="flex flex-wrap gap-1.5">
                        {insight.topTags.map((ts) => (
                            <span key={ts.tag} className="inline-flex items-center gap-1 rounded-full bg-surface-subtle px-2 py-0.5 text-fs-xs text-text-secondary">
                                #{ts.tag}
                                <span className="text-text-tertiary">({ts.count})</span>
                            </span>
                        ))}
                    </div>
                </div>
            )}
        </div>
    );
}


function KnowledgeGuideContent({
    spaceId,
    knowledgeGuide,
    onUpdated,
    t,
}: {
    spaceId: string;
    knowledgeGuide: SpaceKnowledgeGuide | null;
    onUpdated: (kg: SpaceKnowledgeGuide) => void;
    t: (key: string, opts?: Record<string, unknown>) => string;
}) {
    const [isEditing, setIsEditing] = useState(false);
    const [editValue, setEditValue] = useState('');
    const [isRegenerating, setIsRegenerating] = useState(false);

    const handleEdit = useCallback(() => {
        setEditValue(knowledgeGuide?.knowledgeGuideContent ?? '');
        setIsEditing(true);
    }, [knowledgeGuide]);

    const handleSave = useCallback(async () => {
        await invoke('save_space_knowledge_guide', { spaceId, knowledgeGuideContent: editValue });
        onUpdated({ spaceId, knowledgeGuideContent: editValue, knowledgeGuideUpdatedAt: new Date().toISOString() });
        setIsEditing(false);
    }, [spaceId, editValue, onUpdated]);

    const handleRegenerate = useCallback(async () => {
        setIsRegenerating(true);
        try {
            const newContent = await invoke<string>('regenerate_space_knowledge_guide', { spaceId });
            if (newContent) {
                const updated: SpaceKnowledgeGuide = { spaceId, knowledgeGuideContent: newContent, knowledgeGuideUpdatedAt: new Date().toISOString() };
                onUpdated(updated);
            }
        } finally {
            setIsRegenerating(false);
        }
    }, [spaceId, onUpdated]);

    const hasContent = knowledgeGuide?.knowledgeGuideContent && knowledgeGuide.knowledgeGuideContent.trim().length > 0;

    return (
        <div className="space-y-2">
            <div className="flex items-center justify-between">
                {knowledgeGuide?.knowledgeGuideUpdatedAt && (
                    <span className="text-fs-xs text-text-tertiary">
                        {t('space_knowledge_guide.updated_at', { time: new Date(knowledgeGuide.knowledgeGuideUpdatedAt).toLocaleDateString() })}
                    </span>
                )}
                <div className="flex items-center gap-1.5 ml-auto">
                    {!isEditing && (
                        <>
                            <button
                                onClick={handleRegenerate}
                                disabled={isRegenerating}
                                className="flex items-center gap-1 rounded px-2 py-1 text-fs-xs text-text-secondary hover:text-text-primary hover:bg-surface-subtle disabled:opacity-50 transition-colors"
                            >
                                <RefreshCw size={12} className={isRegenerating ? 'animate-spin' : ''} />
                                {isRegenerating ? t('space_knowledge_guide.regenerating') : t('space_knowledge_guide.regenerate')}
                            </button>
                            <button
                                onClick={handleEdit}
                                className="rounded px-2 py-1 text-fs-xs text-text-secondary hover:text-text-primary hover:bg-surface-subtle transition-colors"
                            >
                                {t('space_knowledge_guide.edit')}
                            </button>
                        </>
                    )}
                    {isEditing && (
                        <>
                            <button
                                onClick={() => setIsEditing(false)}
                                className="rounded px-2 py-1 text-fs-xs text-text-secondary hover:text-text-primary hover:bg-surface-subtle transition-colors"
                            >
                                {t('space_knowledge_guide.cancel')}
                            </button>
                            <button
                                onClick={handleSave}
                                className="rounded px-2 py-1 text-fs-xs bg-accent-default text-white hover:opacity-90 transition-opacity"
                            >
                                {t('space_knowledge_guide.save')}
                            </button>
                        </>
                    )}
                </div>
            </div>

            {isEditing ? (
                <textarea
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    className="w-full min-h-[200px] rounded-md border border-stroke-control bg-surface-layer px-3 py-2 text-fs-sm text-text-primary font-mono resize-y focus:outline-none focus:border-accent-default"
                    autoFocus
                />
            ) : hasContent ? (
                <pre className="whitespace-pre-wrap text-fs-sm text-text-primary leading-relaxed font-sans max-h-[400px] overflow-y-auto">
                    {knowledgeGuide!.knowledgeGuideContent}
                </pre>
            ) : (
                <div className="text-fs-sm text-text-tertiary py-2">{t('space_knowledge_guide.empty')}</div>
            )}
        </div>
    );
}
