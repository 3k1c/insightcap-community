import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { ExternalKnowledgeBase, ExternalKbLoadResult } from '../lib/types';
import { Database, Plus, Trash2 } from 'lucide-react';
import { toast } from 'sonner';

export const SettingsPage: React.FC = () => {
    const [activeTab, setActiveTab] = useState<'general' | 'enterprise'>('enterprise');
    const [externalKbs, setExternalKbs] = useState<ExternalKnowledgeBase[]>([]);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        if (activeTab === 'enterprise') {
            loadExternalKbs();
        }
    }, [activeTab]);

    const loadExternalKbs = async () => {
        try {
            const kbs = await invoke<ExternalKnowledgeBase[]>('get_external_kbs');
            setExternalKbs(kbs);
        } catch (error) {
            console.error('Failed to load DBs', error);
            toast.error('無法取得外部知識庫列表');
        }
    };

    const handleAddKb = async () => {
        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'SQLite Database', extensions: ['db', 'sqlite'] }]
            });
            if (selected && typeof selected === 'string') {
                setLoading(true);
                const toastId = toast.loading('正在驗證與掛載外部知識庫...');
                try {
                    const result = await invoke<ExternalKbLoadResult>('load_external_kb', { dbPath: selected });
                    if (result.success) {
                        toast.success('外部知識庫掛載成功！', { id: toastId });
                        loadExternalKbs();
                    } else {
                        toast.error(`掛載失敗: ${result.reason}`, { id: toastId });
                    }
                } catch (e: any) {
                    toast.error(`發生錯誤: ${e.toString()}`, { id: toastId });
                }
            }
        } catch (error) {
            console.error('Add KB failed', error);
            toast.error('開啟檔案對話框失敗');
        } finally {
            setLoading(false);
        }
    };

    const handleRemoveKb = async (id: string, name: string) => {
        if (!window.confirm(`確定要移除外部知識庫 "${name}" 的連線嗎？`)) {
            return;
        }
        try {
            await invoke('remove_external_kb', { id });
            toast.success('已移除知識庫連線');
            loadExternalKbs();
        } catch (error) {
            console.error('Remove KB failed', error);
            toast.error('移除失敗');
        }
    };

    return (
        <div className="flex h-full w-full bg-[var(--ic-bg-base)]">
            <div className="w-64 border-r border-[var(--ic-border)] p-6 flex flex-col gap-2">
                <h2 className="text-xl font-bold mb-4 text-[var(--ic-text-primary)]">設定</h2>
                <button
                    onClick={() => setActiveTab('general')}
                    className={`text-left px-4 py-2 rounded-lg transition-colors ${activeTab === 'general' ? 'bg-[var(--ic-bg-surface)] text-[var(--ic-accent)] font-medium' : 'text-[var(--ic-text-secondary)] hover:bg-[var(--ic-bg-subtle)]'}`}
                >
                    一般設定
                </button>
                <button
                    onClick={() => setActiveTab('enterprise')}
                    className={`text-left px-4 py-2 rounded-lg transition-colors ${activeTab === 'enterprise' ? 'bg-[var(--ic-bg-surface)] text-[var(--ic-accent)] font-medium' : 'text-[var(--ic-text-secondary)] hover:bg-[var(--ic-bg-subtle)]'}`}
                >
                    商業版擴充 (Enterprise)
                </button>
            </div>

            <div className="flex-1 p-8 overflow-y-auto">
                {activeTab === 'general' && (
                    <div className="text-[var(--ic-text-muted)] flex h-full items-center justify-center">一般設定頁面 (建置中)</div>
                )}

                {activeTab === 'enterprise' && (
                    <div className="max-w-4xl">
                        <div className="flex justify-between items-center mb-6">
                            <div>
                                <h3 className="text-2xl font-bold text-[var(--ic-text-primary)]">外部知識庫</h3>
                                <p className="text-[var(--ic-text-secondary)] mt-1">掛載標準化的外部知識庫檔案，啟動企業級混合 RAG 檢索。</p>
                            </div>
                            <button
                                onClick={handleAddKb}
                                disabled={loading}
                                className="flex items-center gap-2 bg-[var(--ic-accent-primary)] text-white px-4 py-2 rounded-xl hover:bg-[var(--ic-accent-hover)] transition-colors disabled:opacity-50"
                            >
                                <Plus className="w-4 h-4" />
                                載入外部資料庫
                            </button>
                        </div>

                        <div className="space-y-4">
                            {externalKbs.length === 0 ? (
                                <div className="text-center py-12 border-2 border-dashed border-[var(--ic-border-default)] rounded-xl text-[var(--ic-text-muted)]">
                                    <Database className="w-8 h-8 mx-auto mb-3 opacity-50" />
                                    <p>尚未掛載任何外部知識庫</p>
                                    <p className="text-xs mt-2">點擊右上方按鈕進行掛載</p>
                                </div>
                            ) : (
                                externalKbs.map(kb => (
                                    <div key={kb.id} className="bg-[var(--ic-bg-surface)] border border-[var(--ic-border)] rounded-xl p-5 flex justify-between items-start">
                                        <div>
                                            <div className="flex items-center gap-3 mb-2">
                                                <Database className="w-5 h-5 text-[var(--ic-accent)]" />
                                                <h4 className="font-bold text-[var(--ic-text-primary)] text-lg">{kb.name}</h4>
                                                <span className={`text-xs px-2 py-1 flex items-center gap-1 rounded-full ${kb.status === 'connected' ? 'bg-green-500/10 text-green-500' : 'bg-red-500/10 text-red-500'}`}>
                                                    <span className={`w-1.5 h-1.5 rounded-full ${kb.status === 'connected' ? 'bg-green-500' : 'bg-red-500'}`}></span>
                                                    {kb.status === 'connected' ? '已連線' : '異常'}
                                                </span>
                                            </div>
                                            <p className="text-[var(--ic-text-secondary)] text-sm mb-3">{kb.description || '由 Knowledge Builder 工具自動建立的標準知識庫。'}</p>
                                            <div className="flex gap-4 text-xs text-[var(--ic-text-muted)]">
                                                <span>路徑: {kb.dbPath}</span>
                                                <span className="opacity-50">|</span>
                                                <span>模型: {kb.embeddingModel} ({kb.embeddingDimension}維)</span>
                                            </div>
                                        </div>
                                        <button
                                            onClick={() => handleRemoveKb(kb.id, kb.name)}
                                            className="p-2 text-[var(--ic-text-muted)] hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors"
                                            title="移除連線"
                                        >
                                            <Trash2 className="w-5 h-5" />
                                        </button>
                                    </div>
                                ))
                            )}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
};
