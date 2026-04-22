import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './design-system/index.css';

import './i18n';

import { invoke } from '@tauri-apps/api/core';

(function initZoom() {
    const saved = localStorage.getItem('zoomLevel');
    let level = saved !== null ? Number(saved) : 0;

    function apply(l: number) {
        const factor = Math.pow(1.2, l);
        invoke('set_zoom', { factor }).then(() => {
            const root = document.getElementById('root');
            if (root) {
                root.style.width = `${window.innerWidth}px`;
                root.style.height = `${window.innerHeight}px`;
            }
        });
        localStorage.setItem('zoomLevel', String(l));
    }

    apply(level);

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
