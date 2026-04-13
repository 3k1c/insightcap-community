import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './design-system/index.css';

// 初始化 i18n（必須在 React 掛載前）
import './i18n';

// UI Zoom (Ctrl+= / Ctrl+- / Ctrl+0) — 使用 Tauri set_zoom，隨視窗大小正確縮放
import { invoke } from '@tauri-apps/api/core';

(function initZoom() {
    const saved = localStorage.getItem('zoomLevel');
    let level = saved !== null ? Number(saved) : 0;

    function apply(l: number) {
        const factor = Math.pow(1.2, l);
        invoke('set_zoom', { factor }).then(() => {
            // set_zoom 改變了 viewport 尺寸，強制 #root 填滿新的 viewport
            const root = document.getElementById('root');
            if (root) {
                root.style.width = `${window.innerWidth}px`;
                root.style.height = `${window.innerHeight}px`;
            }
        });
        localStorage.setItem('zoomLevel', String(l));
    }

    apply(level);

    // 視窗 resize 後同步更新 #root 尺寸
    window.addEventListener('resize', () => {
        const root = document.getElementById('root');
        if (root && root.style.width) {
            root.style.width = `${window.innerWidth}px`;
            root.style.height = `${window.innerHeight}px`;
        }
    });

    window.addEventListener('keydown', (e) => {
        if (!e.ctrlKey) return;
        if (e.key === '=' || e.key === '+') {
            e.preventDefault();
            level = Math.min(level + 1, 5);
            apply(level);
        } else if (e.key === '-') {
            e.preventDefault();
            level = Math.max(level - 1, -5);
            apply(level);
        } else if (e.key === '0') {
            e.preventDefault();
            level = 0;
            apply(0);
        }
    });
})();

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <App />
    </React.StrictMode>
);
