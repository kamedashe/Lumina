import React from 'react';

export interface Message {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export interface ChatSession {
  id: string;
  title: string;
  date: number;
  messages: Message[];
}

export interface CommandSuggestion {
  id: string;
  icon: React.ReactNode;
  label: string;
  action: () => void;
  description: string;
}

// Helper for Tauri Invoke to prevent TS errors in non-Tauri envs (simulated)
declare global {
  interface Window {
    __TAURI__: {
      invoke: <T>(cmd: string, args?: unknown) => Promise<T>;
    };
  }
}