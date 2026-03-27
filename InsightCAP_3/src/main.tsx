import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './design-system/index.css';

// 初始化 i18n（必須在 React 掛載前）
import './i18n';

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <App />
    </React.StrictMode>
);
