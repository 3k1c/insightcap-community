import React from 'react';
import { useUiStore } from '../../stores/uiStore';
import { MessageSquare, Database, Settings } from 'lucide-react';
import { ChatPage } from '../../pages/ChatPage';
import { KnowledgePage } from '../../pages/KnowledgePage';
import { SettingsPage } from '../../pages/SettingsPage';

export const MainLayout: React.FC = () => {
    const { activePage, setActivePage } = useUiStore();

    return (
        <div className="flex w-full h-screen bg-[var(--ic-bg-base)] text-[var(--ic-text-primary)]">
            {/* Global Slim Sidebar */}
            <div className="w-16 border-r border-[var(--ic-border)] bg-[var(--ic-bg-surface)] flex flex-col items-center py-4 gap-4 shrink-0 z-50">
                <button
                    onClick={() => setActivePage('chat')}
                    className={`p-2.5 rounded-xl transition-all ${activePage === 'chat' ? 'bg-[var(--ic-accent-primary)] text-white shadow-md' : 'text-[var(--ic-text-secondary)] hover:text-[var(--ic-text-primary)] hover:bg-[var(--ic-bg-subtle)]'}`}
                    title="對話"
                >
                    <MessageSquare className="w-5 h-5" />
                </button>
                <button
                    onClick={() => setActivePage('knowledge')}
                    className={`p-2.5 rounded-xl transition-all ${activePage === 'knowledge' ? 'bg-[var(--ic-accent-primary)] text-white shadow-md' : 'text-[var(--ic-text-secondary)] hover:text-[var(--ic-text-primary)] hover:bg-[var(--ic-bg-subtle)]'}`}
                    title="記憶庫"
                >
                    <Database className="w-5 h-5" />
                </button>
                <div className="flex-1" />
                <button
                    onClick={() => setActivePage('settings')}
                    className={`p-2.5 rounded-xl transition-all ${activePage === 'settings' ? 'bg-[var(--ic-accent-primary)] text-white shadow-md' : 'text-[var(--ic-text-secondary)] hover:text-[var(--ic-text-primary)] hover:bg-[var(--ic-bg-subtle)]'}`}
                    title="設定"
                >
                    <Settings className="w-5 h-5" />
                </button>
            </div>

            {/* Main Content Area */}
            <div className="flex-1 flex overflow-hidden">
                {activePage === 'chat' && <ChatPage />}
                {activePage === 'knowledge' && <KnowledgePage />}
                {activePage === 'settings' && <SettingsPage />}
            </div>
        </div>
    );
};
