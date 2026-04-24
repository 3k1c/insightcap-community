import React, { useState } from 'react';
import { useUiStore } from '../../stores/uiStore';
import { useT } from '../../hooks/useT';
import { Database, MessageSquare, Settings, Calendar } from 'lucide-react';
import { ChatPage } from '../../pages/ChatPage';
import { RepositoryPage } from '../../pages/RepositoryPage';
import { SchedulePage } from '../../pages/SchedulePage';
import { SettingsPage } from '../../pages/SettingsPage';
import { PendingConfirmDrawer, usePendingConfirmCount } from '../memory/PendingConfirmDrawer';

export const MainLayout: React.FC = () => {
    const { activePage, setActivePage, toggleSidebar, isChatHidden, toggleChatHidden, isSidebarOpen } = useUiStore();
    const t = useT();
    const pendingCount = usePendingConfirmCount();
    const [drawerOpen, setDrawerOpen] = useState(false);

    return (
        <div className="flex flex-col w-full h-screen bg-surface-base text-text-primary">
            {/* Top Nav */}
            <div className="h-12 border-b border-stroke-divider bg-surface-layer flex items-center px-4 gap-1 shrink-0 z-50">
                <button
                    onClick={() => {
                        setActivePage('chat');
                        if (isChatHidden) {
                            toggleChatHidden();
                            if (!isSidebarOpen) toggleSidebar();
                        }
                    }}
                    onDoubleClick={() => { setActivePage('chat'); toggleSidebar(); }}
                    className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-fs-sm transition-all ${activePage === 'chat' ? 'bg-accent-default text-white shadow-sm' : 'text-text-secondary hover:text-text-primary hover:bg-surface-subtle'}`}
                >
                    <MessageSquare className="w-4 h-4" />
                    <span>{t('nav.chat')}</span>
                </button>
                <div className="relative">
                    <button
                        onClick={() => setActivePage('repository')}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-fs-sm transition-all ${activePage === 'repository' ? 'bg-accent-default text-white shadow-sm' : 'text-text-secondary hover:text-text-primary hover:bg-surface-subtle'}`}
                    >
                        <Database className="w-4 h-4" />
                        <span>{t('nav.repository')}</span>
                    </button>
                    {pendingCount > 0 && (
                        <button
                            onClick={(e) => { e.stopPropagation(); setDrawerOpen(true); }}
                            className="absolute -top-1 -right-1 min-w-[16px] h-4 rounded-full bg-red-500 flex items-center justify-center px-1 text-[10px] font-bold text-white leading-none hover:bg-red-600 transition-colors"
                        >
                            {pendingCount > 99 ? '99+' : pendingCount}
                        </button>
                    )}
                </div>
                <button
                    onClick={() => setActivePage('schedule')}
                    className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-fs-sm transition-all ${activePage === 'schedule' ? 'bg-accent-default text-white shadow-sm' : 'text-text-secondary hover:text-text-primary hover:bg-surface-subtle'}`}
                >
                    <Calendar className="w-4 h-4" />
                    <span>{t('nav.schedule')}</span>
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
                {activePage === 'schedule' && <SchedulePage />}
                {activePage === 'settings' && <SettingsPage />}
            </div>

            <PendingConfirmDrawer
                open={drawerOpen}
                onClose={() => setDrawerOpen(false)}
            />
        </div>
    );
};
