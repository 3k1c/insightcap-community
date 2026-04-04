import React from 'react';
import { useUiStore } from '../../stores/uiStore';
import { useT } from '../../hooks/useT';
import { Database, MessageSquare, Settings } from 'lucide-react';
import { ChatPage } from '../../pages/ChatPage';
import { RepositoryPage } from '../../pages/RepositoryPage';
import { SettingsPage } from '../../pages/SettingsPage';

export const MainLayout: React.FC = () => {
    const { activePage, setActivePage, toggleSidebar } = useUiStore();
    const t = useT();

    return (
        <div className="flex flex-col w-full h-screen bg-surface-base text-text-primary">
            {/* Top Nav */}
            <div className="h-12 border-b border-stroke-divider bg-surface-layer flex items-center px-4 gap-1 shrink-0 z-50">
                <button
                    onClick={() => setActivePage('chat')}
                    onDoubleClick={() => { setActivePage('chat'); toggleSidebar(); }}
                    className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-fs-sm transition-all ${activePage === 'chat' ? 'bg-accent-default text-white shadow-sm' : 'text-text-secondary hover:text-text-primary hover:bg-surface-subtle'}`}
                >
                    <MessageSquare className="w-4 h-4" />
                    <span>{t('nav.chat')}</span>
                </button>
                <button
                    onClick={() => setActivePage('repository')}
                    className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-fs-sm transition-all ${activePage === 'repository' ? 'bg-accent-default text-white shadow-sm' : 'text-text-secondary hover:text-text-primary hover:bg-surface-subtle'}`}
                >
                    <Database className="w-4 h-4" />
                    <span>{t('nav.repository')}</span>
                </button>
                <div className="flex-1" />
                <button
                    onClick={() => setActivePage('settings')}
                    className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-fs-sm transition-all ${activePage === 'settings' ? 'bg-accent-default text-white shadow-sm' : 'text-text-secondary hover:text-text-primary hover:bg-surface-subtle'}`}
                >
                    <Settings className="w-4 h-4" />
                    <span>{t('nav.settings')}</span>
                </button>
            </div>

            {/* Main Content Area */}
            <div className="flex-1 flex overflow-hidden">
                {activePage === 'chat' && <ChatPage />}
                {activePage === 'repository' && <RepositoryPage />}
                {activePage === 'settings' && <SettingsPage />}
            </div>
        </div>
    );
};
