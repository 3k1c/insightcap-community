import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
    invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('./pages/SetupPage', () => ({
    SetupPage: () => <div>setup page</div>,
}));

vi.mock('./pages/LoginPage', () => ({
    LoginPage: () => <div>login page</div>,
}));

vi.mock('./pages/MigratePage', () => ({
    MigratePage: () => <div>migrate page</div>,
}));

vi.mock('./components/layout/MainLayout', () => ({
    MainLayout: () => <div>main layout</div>,
}));

vi.mock('./pages/QuickCapturePage', () => ({
    QuickCapturePage: () => <div>quick capture</div>,
}));

vi.mock('./components/memory/DecisionToast', () => ({
    DecisionReviewToast: () => null,
}));

vi.mock('./components/memory/ReminderToast', () => ({
    ReminderToast: () => null,
}));

describe('App startup loading screen', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        mockInvoke.mockReturnValue(new Promise(() => undefined));
    });

    it('shows first-startup guidance while initialization is pending', () => {
        render(<App />);

        expect(screen.getByText('正在啟動 InsightCAP')).toBeInTheDocument();
        expect(screen.getByText('首次啟動可能需要幾秒鐘，請稍候。')).toBeInTheDocument();
    });

    it('uses one startup command instead of chaining settings and auth commands', async () => {
        mockInvoke.mockImplementation((command: string) => {
            if (command === 'get_startup_status') {
                return Promise.resolve({
                    kbPath: 'C:\\Data\\InsightCAP',
                    isSetup: true,
                    autoLogin: true,
                    isMigrated: false,
                });
            }
            return Promise.reject(new Error(`unexpected command: ${command}`));
        });

        render(<App />);

        expect(await screen.findByText('main layout')).toBeInTheDocument();
        const commands = mockInvoke.mock.calls.map(([command]) => command);
        expect(commands).toEqual(['get_startup_status']);
    });

    it('opens migration recovery when startup reports that the active knowledge base cannot auto-open', async () => {
        mockInvoke.mockResolvedValue({
            kbPath: 'D:\\ck\\testing\\data',
            isSetup: true,
            autoLogin: false,
            isMigrated: true,
        });

        render(<App />);

        expect(await screen.findByText('migrate page')).toBeInTheDocument();
    });
});
