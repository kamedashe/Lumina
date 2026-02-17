import React from 'react';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { motion } from 'framer-motion';

export const WindowControls = () => {
    const appWindow = getCurrentWindow();
    return (
        <div className="flex items-center gap-2 z-50 ml-2">
            <button onClick={() => appWindow.minimize()} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/10 text-gray-400 hover:text-white transition-colors group">
                <Minus size={16} />
            </button>
            <button onClick={() => appWindow.toggleMaximize()} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/10 text-gray-400 hover:text-white transition-colors group">
                <Square size={14} />
            </button>
            <button onClick={() => appWindow.close()} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors">
                <X size={18} />
            </button>
        </div>
    );
};

export const LuminaLogo: React.FC<{ className?: string }> = ({ className = "w-8 h-8" }) => (
    <div className={`relative flex items-center justify-center ${className}`}>
        <div className="absolute inset-0 bg-purple-600 blur-[25px] opacity-60 animate-pulse"></div>
        <svg viewBox="0 0 100 100" className="relative w-full h-full z-10 drop-shadow-[0_0_15px_rgba(168,85,247,0.5)]">
            <path d="M50 10 L90 50 L50 90 L10 50 Z" stroke="white" strokeWidth="6" fill="transparent" />
            <circle cx="50" cy="50" r="6" fill="#a855f7" className="animate-pulse" />
        </svg>
    </div>
);

export const TypingIndicator = () => (
    <div className="flex items-center gap-3 p-4 bg-[#1e1e24]/90 rounded-2xl w-fit border border-white/5 shadow-xl">
        <div className="flex gap-1">
            <motion.div className="w-2 h-2 bg-purple-500 rounded-full" animate={{ y: [0, -5, 0] }} transition={{ duration: 0.6, repeat: Infinity, delay: 0 }} />
            <motion.div className="w-2 h-2 bg-purple-500 rounded-full" animate={{ y: [0, -5, 0] }} transition={{ duration: 0.6, repeat: Infinity, delay: 0.2 }} />
            <motion.div className="w-2 h-2 bg-purple-500 rounded-full" animate={{ y: [0, -5, 0] }} transition={{ duration: 0.6, repeat: Infinity, delay: 0.4 }} />
        </div>
        <span className="text-xs font-medium text-gray-400 animate-pulse">Lumina thinking...</span>
    </div>
);
