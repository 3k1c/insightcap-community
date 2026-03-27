import React, { useState } from 'react';
import { ChevronDown, ChevronUp, FileText, Image, Globe } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { SourceItem, CaptureItem } from '../../stores/knowledgeStore';

interface SourceCardProps {
    source: SourceItem;
}

export const SourceCard: React.FC<SourceCardProps> = ({ source }) => {
    const [expanded, setExpanded] = useState(false);
    const [captures, setCaptures] = useState<CaptureItem[]>([]);
    const [loading, setLoading] = useState(false);

    const toggleExpand = async () => {
        if (!expanded && captures.length === 0) {
            setLoading(true);
            try {
                // 擴充後的 get_captures 支援傳入 source_id
                const res = await invoke<CaptureItem[]>('get_captures', { sourceId: source.id, limit: 50 });
                setCaptures(res);
            } catch (e) {
                console.error('Failed to fetch captures', e);
            } finally {
                setLoading(false);
            }
        }
        setExpanded(!expanded);
    };

    const getIcon = () => {
        switch (source.type) {
            case 'document':
            case 'editor':
                return <FileText className="w-5 h-5 text-blue-500" />;
            case 'image':
                return <Image className="w-5 h-5 text-purple-500" />;
            case 'url':
                return <Globe className="w-5 h-5 text-green-500" />;
            default:
                return <FileText className="w-5 h-5 text-[var(--ic-text-secondary)]" />;
        }
    };

    return (
        <div className="bg-[var(--ic-bg-surface)] border border-[var(--ic-border)] rounded-xl shadow-sm hover:border-[var(--ic-border-strong)] transition-all overflow-hidden">
            <div
                className="p-4 cursor-pointer flex items-start gap-4"
                onClick={toggleExpand}
            >
                <div className="mt-1 shrink-0 p-2 bg-[var(--ic-bg-subtle)] rounded-lg">
                    {getIcon()}
                </div>
                <div className="flex-1 min-w-0">
                    <div className="flex flex-wrap items-center gap-2 mb-1">
                        <h3 className="font-semibold text-base text-[var(--ic-text-primary)] truncate" title={source.title || '無標題'}>
                            {source.title || '無標題'}
                        </h3>
                        {source.type && (
                            <span className="text-[10px] px-2 py-0.5 rounded-full bg-[var(--ic-bg-subtle)] text-[var(--ic-text-muted)] uppercase tracking-wider border border-[var(--ic-border)]">
                                {source.type}
                            </span>
                        )}
                    </div>
                    {(source.filePath || source.url) && (
                        <div className="text-xs text-[var(--ic-text-muted)] mb-2 truncate" title={source.filePath || source.url}>
                            {source.filePath || source.url}
                        </div>
                    )}
                    <p className="text-sm text-[var(--ic-text-secondary)] line-clamp-2 leading-relaxed">
                        {source.contentPreview || '未提供預覽內容'}
                    </p>
                    <div className="mt-4 flex items-center justify-between">
                        <span className="text-xs font-medium text-[var(--ic-text-muted)]">
                            {new Date(source.capturedAt).toLocaleString()}
                        </span>
                        <div className="flex items-center gap-1 text-[var(--ic-text-muted)] text-xs font-medium bg-[var(--ic-bg-subtle)] px-2 py-1 rounded-md hover:bg-[var(--ic-border)] transition-colors">
                            {expanded ? <><ChevronUp className="w-3.5 h-3.5" /> 隱藏知識點</> : <><ChevronDown className="w-3.5 h-3.5" /> 檢視知識點</>}
                        </div>
                    </div>
                </div>
            </div>

            {/* 展開的 Captures 區域 */}
            {expanded && (
                <div className="bg-[var(--ic-bg-base)] border-t border-[var(--ic-border)] p-4">
                    {loading ? (
                        <div className="text-center py-4 text-xs text-[var(--ic-text-muted)]">載入中...</div>
                    ) : captures.length > 0 ? (
                        <div className="space-y-3">
                            {captures.map(cap => (
                                <div key={cap.id} className="p-3 bg-[var(--ic-bg-surface)] border border-[var(--ic-border)] rounded-lg text-sm text-[var(--ic-text-primary)] leading-relaxed shadow-sm">
                                    {cap.cleanContent}
                                </div>
                            ))}
                        </div>
                    ) : (
                        <div className="text-center py-4 text-xs text-[var(--ic-text-muted)]">無相關知識片段</div>
                    )}
                </div>
            )}
        </div>
    );
};
