import React from 'react';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const WindowControls: React.FC = () => {
    const appWindow = getCurrentWindow();
    return (
        <div className="flex items-center gap-1">
            <button
                onClick={() => appWindow.minimize()}
                className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated transition-colors"
                title="Свернуть"
            >
                <Minus size={15} />
            </button>
            <button
                onClick={() => appWindow.toggleMaximize()}
                className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated transition-colors"
                title="Развернуть"
            >
                <Square size={12} />
            </button>
            <button
                onClick={() => appWindow.close()}
                className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-white hover:bg-red-500/90 transition-colors"
                title="Закрыть"
            >
                <X size={16} />
            </button>
        </div>
    );
};

export const LuminaLogo: React.FC<{ className?: string }> = ({ className = 'w-7 h-7' }) => (
    <svg viewBox="0 0 32 32" className={className} fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect x="1" y="1" width="30" height="30" rx="8" fill="rgb(var(--accent))" />
        <path
            d="M16 7.5 L23.5 16 L16 24.5 L8.5 16 Z"
            stroke="rgb(var(--accent-fg))"
            strokeWidth="2.2"
            strokeLinejoin="round"
            fill="none"
        />
        <circle cx="16" cy="16" r="2.6" fill="rgb(var(--accent-fg))" />
    </svg>
);

export const TypingIndicator: React.FC = () => (
    <div className="flex items-center gap-1.5 py-1">
        {[0, 0.15, 0.3].map((delay, i) => (
            <span
                key={i}
                className="w-1.5 h-1.5 rounded-full bg-subtle animate-bounce"
                style={{ animationDelay: `${delay}s`, animationDuration: '0.9s' }}
            />
        ))}
    </div>
);
