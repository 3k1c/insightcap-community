import React, { useEffect, useState } from 'react';
import { Calendar, Clock, CheckCircle2, AlertCircle, FileText, Milestone, History, ChevronRight } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { Dialog } from '../ui/Dialog';
import { Badge } from '../ui/Badge';

interface ProjectTimelineModalProps {
    isOpen: boolean;
    onClose: () => void;
    projectId: string;
    projectName: string;
}

interface Reminder {
    id: string;
    title: string;
    description: string | null;
    eventDate: string | null;
    eventTime: string | null;
    eventType: string; // meeting, deliverable, event, appointment
    status: string; // active, completed, dismissed, expired
}

interface KeyPoint {
    id: string;
    content: string;
    type: string; // pattern, log
    createdAt: string;
}

interface TimelineData {
    reminders: Reminder[];
    keyPoints: KeyPoint[];
}

export const ProjectTimelineModal: React.FC<ProjectTimelineModalProps> = ({
    isOpen,
    onClose,
    projectId,
    projectName
}) => {
    const [data, setData] = useState<TimelineData | null>(null);
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
        if (isOpen && projectId) {
            handleLoadData();
        }
    }, [isOpen, projectId]);

    const handleLoadData = async () => {
        setIsLoading(true);
        try {
            const res = await invoke<TimelineData>('get_project_timeline', { projectId });
            setData(res);
        } catch (error) {
            console.error('Failed to load project timeline:', error);
        } finally {
            setIsLoading(false);
        }
    };

    const sortedTimeline = () => {
        if (!data) return [];
        // Combine reminders and key points into a chronological list if possible
        // For now, let's keep them separate or just list them
        return data.reminders.sort((a, b) => {
            const dateA = a.eventDate || '9999-99-99';
            const dateB = b.eventDate || '9999-99-99';
            return dateA.localeCompare(dateB);
        });
    };

    const getEventIcon = (type: string) => {
        switch (type) {
            case 'meeting': return <Calendar className="w-3.5 h-3.5" />;
            case 'deliverable': return <CheckCircle2 className="w-3.5 h-3.5" />;
            case 'appointment': return <Clock className="w-3.5 h-3.5" />;
            default: return <AlertCircle className="w-3.5 h-3.5" />;
        }
    };

    const getEventColor = (type: string) => {
        switch (type) {
            case 'meeting': return 'bg-blue-500/10 text-blue-500 border-blue-500/20';
            case 'deliverable': return 'bg-green-500/10 text-green-500 border-green-500/20';
            case 'appointment': return 'bg-purple-500/10 text-purple-500 border-purple-500/20';
            default: return 'bg-amber-500/10 text-amber-500 border-amber-500/20';
        }
    };

    return (
        <Dialog
            open={isOpen}
            onClose={onClose}
            title={`項目時間表: ${projectName}`}
            size="lg"
        >
            <div className="flex flex-col h-[500px] overflow-hidden">
                <div className="flex-1 overflow-y-auto px-1 pr-2 space-y-6 custom-scrollbar">

                    {/* 提醒事項與里程碑 */}
                    <section>
                        <div className="flex items-center gap-2 mb-4">
                            <Milestone className="w-4 h-4 text-accent-default" />
                            <h3 className="text-fs-sm font-semibold text-text-secondary uppercase tracking-wider">主要里程碑與提醒</h3>
                        </div>

                        {isLoading ? (
                            <div className="py-8 text-center text-text-tertiary">載入中...</div>
                        ) : !data?.reminders.length ? (
                            <div className="py-6 text-center text-text-tertiary bg-surface-soft rounded-xl border border-dashed border-stroke-divider">
                                尚無設定提醒事項
                            </div>
                        ) : (
                            <div className="space-y-3 relative before:absolute before:left-[11px] before:top-2 before:bottom-2 before:w-0.5 before:bg-stroke-divider/50">
                                {sortedTimeline().map((rem) => (
                                    <div key={rem.id} className="relative pl-8 group">
                                        <div className={`absolute left-0 top-1.5 w-6 h-6 rounded-full flex items-center justify-center border-2 border-surface-base z-10 ${rem.status === 'completed' ? 'bg-green-500' : 'bg-accent-default shadow-sm'}`}>
                                            {rem.status === 'completed' ? <CheckCircle2 className="w-3 h-3 text-white" /> : <Clock className="w-3 h-3 text-white" />}
                                        </div>
                                        <div className="p-3 rounded-xl bg-surface-soft border border-stroke-divider hover:border-accent-default/30 transition-all group-hover:shadow-sm">
                                            <div className="flex items-start justify-between gap-2 mb-1">
                                                <h4 className={`text-fs-sm font-medium ${rem.status === 'completed' ? 'text-text-tertiary line-through' : 'text-text-primary'}`}>
                                                    {rem.title}
                                                </h4>
                                                <Badge variant="outline" className={`shrink-0 text-[10px] py-0 px-1.5 ${getEventColor(rem.eventType)}`}>
                                                    {rem.eventType === 'meeting' ? '會議' : rem.eventType === 'deliverable' ? '交付' : '提醒'}
                                                </Badge>
                                            </div>
                                            <div className="flex items-center gap-3 text-[11px] text-text-tertiary">
                                                <span className="flex items-center gap-1">
                                                    <Calendar className="w-3 h-3" />
                                                    {rem.eventDate || '未定日期'}
                                                </span>
                                                {rem.eventTime && (
                                                    <span className="flex items-center gap-1">
                                                        <Clock className="w-3 h-3" />
                                                        {rem.eventTime}
                                                    </span>
                                                )}
                                            </div>
                                            {rem.description && (
                                                <p className="mt-2 text-fs-xs text-text-secondary leading-relaxed line-clamp-2">
                                                    {rem.description}
                                                </p>
                                            )}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </section>

                    {/* 項目要點 */}
                    <section>
                        <div className="flex items-center gap-2 mb-4">
                            <History className="w-4 h-4 text-purple-400" />
                            <h3 className="text-fs-sm font-semibold text-text-secondary uppercase tracking-wider">關鍵見解與紀錄</h3>
                        </div>

                        {!data?.keyPoints.length ? (
                            <div className="py-6 text-center text-text-tertiary bg-surface-soft rounded-xl border border-dashed border-stroke-divider">
                                尚無關鍵點紀錄
                            </div>
                        ) : (
                            <div className="grid grid-cols-1 gap-3">
                                {data.keyPoints.map((point) => (
                                    <div key={point.id} className="p-3 rounded-xl bg-surface-subtle border border-stroke-divider hover:bg-surface-base transition-colors group">
                                        <div className="flex items-center gap-2 mb-2">
                                            <div className={`p-1 rounded ${point.type === 'pattern' ? 'bg-purple-500/10 text-purple-500' : 'bg-amber-500/10 text-amber-500'}`}>
                                                <FileText className="w-3 h-3" />
                                            </div>
                                            <span className="text-[10px] font-bold uppercase tracking-wider opacity-60">
                                                {point.type === 'pattern' ? '重複模式' : '風險紀錄'}
                                            </span>
                                            <span className="ml-auto text-[10px] text-text-tertiary">
                                                {new Date(point.createdAt).toLocaleDateString()}
                                            </span>
                                        </div>
                                        <p className="text-fs-sm text-text-secondary leading-relaxed">
                                            {point.content}
                                        </p>
                                    </div>
                                ))}
                            </div>
                        )}
                    </section>
                </div>

                <div className="mt-6 pt-4 border-t border-stroke-divider flex justify-end">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 text-fs-sm font-medium text-text-secondary hover:text-text-primary transition-colors"
                    >
                        關閉
                    </button>
                </div>
            </div>
        </Dialog>
    );
};
