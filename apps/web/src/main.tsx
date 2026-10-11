import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { styles } from '@agent-harness/ui-components';

// 注入全局样式
const styleTag = document.createElement('style');
styleTag.textContent = styles;
document.head.appendChild(styleTag);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
