import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
    Send, Settings, Plus, Trash2, X, PanelLeft, MessageSquare, Blocks,
    Paperclip, FileText, Sun, Moon, Cpu, Code, Sparkles, PenTool, Play,
} from 'lucide-react';
import './style.css';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

import { WindowControls, LuminaLogo, TypingIndicator } from './src/components/UI.tsx';
import { ChatMessage } from './src/components/ChatMessage.tsx';
import { ProviderSettings } from './src/components/ProviderSettings.tsx';
import { aiService } from './src/services/ai.ts';
import { pluginService } from './src/services/plugins.ts';
import { SYSTEM_PROMPT, defaultProvider, resolveEmbeddingProvider } from './src/constants.ts';
import { Message, ChatSession, ProviderConfig, WireMessage, ToolInvocation, VectorStoreConfig } from './src/types/index.ts';

type Theme = 'light' | 'dark';

const App: React.FC = () => {
    const [input, setInput] = useState('');
    const [messages, setMessages] = useState<Message[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [history, setHistory] = useState<ChatSession[]>([]);
    const [statusMsg, setStatusMsg] = useState<string | null>(null);
    const [currentChatId, setCurrentChatId] = useState<number | null>(null);
    const [attachments, setAttachments] = useState<string[]>([]);

    const [providers, setProviders] = useState<ProviderConfig[]>([]);
    const [activeProviderId, setActiveProviderId] = useState<string>('');
    const [store, setStore] = useState<VectorStoreConfig>({ kind: 'sqlite_vec' });
    const [useGraph, setUseGraph] = useState(false);

    const [isSidebarOpen, setIsSidebarOpen] = useState(true);
    const [showSettings, setShowSettings] = useState(false);
    const [showPlugins, setShowPlugins] = useState(false);
    const [theme, setTheme] = useState<Theme>('dark');

    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    const activeProvider = providers.find((p) => p.id === activeProviderId) ?? providers[0];

    // --- ЗАГРУЗКА СОСТОЯНИЯ ---
    useEffect(() => {
        const savedHistory = localStorage.getItem('lumina_history');
        if (savedHistory) {
            try {
                setHistory(JSON.parse(savedHistory));
            } catch { /* ignore */ }
        }

        const savedTheme = (localStorage.getItem('lumina_theme') as Theme) || 'dark';
        applyTheme(savedTheme);
        setTheme(savedTheme);

        const savedProviders = localStorage.getItem('lumina_providers');
        let loaded: ProviderConfig[] = [];
        if (savedProviders) {
            try {
                loaded = JSON.parse(savedProviders);
            } catch { /* ignore */ }
        }
        if (loaded.length === 0) loaded = [defaultProvider()];
        setProviders(loaded);

        const savedActive = localStorage.getItem('lumina_active_provider');
        setActiveProviderId(savedActive && loaded.some((p) => p.id === savedActive) ? savedActive : loaded[0].id);

        setUseGraph(localStorage.getItem('lumina_use_graph') === 'true');

        const savedStore = localStorage.getItem('lumina_vector_store');
        if (savedStore) {
            try {
                setStore(JSON.parse(savedStore));
            } catch { /* ignore */ }
        }
    }, []);

    // --- ПЕРСИСТ ---
    useEffect(() => {
        if (history.length > 0) localStorage.setItem('lumina_history', JSON.stringify(history));
    }, [history]);

    useEffect(() => {
        if (providers.length > 0) localStorage.setItem('lumina_providers', JSON.stringify(providers));
    }, [providers]);

    useEffect(() => {
        if (activeProviderId) localStorage.setItem('lumina_active_provider', activeProviderId);
    }, [activeProviderId]);

    useEffect(() => {
        localStorage.setItem('lumina_vector_store', JSON.stringify(store));
    }, [store]);

    useEffect(() => {
        localStorage.setItem('lumina_use_graph', String(useGraph));
    }, [useGraph]);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages, statusMsg]);

    const applyTheme = (t: Theme) => document.documentElement.setAttribute('data-theme', t);

    const toggleTheme = () => {
        const next: Theme = theme === 'dark' ? 'light' : 'dark';
        setTheme(next);
        applyTheme(next);
        localStorage.setItem('lumina_theme', next);
    };

    const handleAttach = async () => {
        try {
            const selected = await openDialog({
                multiple: true,
                filters: [{ name: 'Documents', extensions: ['pdf', 'txt', 'md', 'json', 'csv', 'rs', 'ts', 'tsx', 'js', 'py'] }],
            });
            if (selected) {
                const files = Array.isArray(selected) ? selected : [selected];
                const paths = files.map((f: any) => (typeof f === 'string' ? f : f.path));
                setAttachments((prev) => [...prev, ...paths]);
            }
        } catch (e) {
            console.error(e);
        }
    };

    /** История сообщений в формат бэкенда: только роль и текст (инструменты — display-only). */
    const toWire = (msgs: Message[]): WireMessage[] =>
        msgs.map((m) => ({ role: m.role, content: m.content }));

    const handleSend = useCallback(
        async (textOverride?: string) => {
            const text = (textOverride ?? input).trim();
            if ((!text && attachments.length === 0) || isLoading) return;
            if (!activeProvider) {
                setShowSettings(true);
                return;
            }

            setInput('');
            setIsLoading(true);
            setStatusMsg(null);

            const userMsg: Message = { role: 'user', content: text };
            const assistantMsg: Message = { role: 'assistant', content: '', tools: [] };
            const baseMessages = [...messages, userMsg];
            // Плейсхолдер ассистента, в который стримим ответ.
            setMessages([...baseMessages, assistantMsg]);
            const assistantIndex = baseMessages.length;

            // Заводим или обновляем сессию сразу, чтобы не потерять при перезагрузке.
            let chatId = currentChatId;
            const isNew = chatId === null;
            if (chatId === null) {
                chatId = Date.now();
                setCurrentChatId(chatId);
                setHistory((prev) => [
                    {
                        id: chatId!,
                        title: text.slice(0, 40) || 'Новый чат',
                        date: Date.now(),
                        messages: baseMessages,
                        providerId: activeProvider.id,
                    },
                    ...prev,
                ]);
            }

            const sentAttachments = attachments;
            setAttachments([]);

            const patchAssistant = (fn: (m: Message) => Message) =>
                setMessages((prev) => prev.map((m, i) => (i === assistantIndex ? fn(m) : m)));

            try {
                await aiService.runAgent({
                    config: activeProvider,
                    store,
                    embeddingProvider: resolveEmbeddingProvider(activeProvider, providers),
                    useGraph,
                    system: SYSTEM_PROMPT,
                    messages: toWire(baseMessages),
                    attachments: sentAttachments,
                    onEvent: (event) => {
                        switch (event.type) {
                            case 'text':
                                patchAssistant((m) => ({ ...m, content: m.content + event.delta }));
                                break;
                            case 'status':
                                setStatusMsg(event.message);
                                break;
                            case 'tool_call_start':
                                setStatusMsg(`Инструмент: ${event.name}…`);
                                break;
                            case 'tool_result': {
                                const inv: ToolInvocation = {
                                    id: event.id,
                                    name: event.name,
                                    content: event.content,
                                    isError: event.is_error,
                                };
                                patchAssistant((m) => ({ ...m, tools: [...(m.tools ?? []), inv] }));
                                setStatusMsg(null);
                                break;
                            }
                            case 'error':
                                patchAssistant((m) => ({
                                    ...m,
                                    content: m.content + (m.content ? '\n\n' : '') + `⚠️ ${event.message}`,
                                }));
                                break;
                            case 'done':
                                setStatusMsg(null);
                                break;
                        }
                    },
                });
            } catch (e) {
                patchAssistant((m) => ({ ...m, content: m.content + `\n\n⚠️ ${e}` }));
            } finally {
                setIsLoading(false);
                setStatusMsg(null);
                // Синхронизируем финальное состояние в историю.
                // setHistory откладываем в микротаск, чтобы не обновлять состояние
                // прямо внутри апдейтера setMessages.
                setMessages((finalMessages) => {
                    queueMicrotask(() =>
                        setHistory((prev) =>
                            prev.map((c) => (c.id === chatId ? { ...c, messages: finalMessages } : c)),
                        ),
                    );
                    return finalMessages;
                });
                if (isNew) inputRef.current?.focus();
            }
        },
        [input, attachments, isLoading, messages, currentChatId, activeProvider, providers, store, useGraph],
    );

    const startNewChat = () => {
        setMessages([]);
        setCurrentChatId(null);
        setAttachments([]);
        inputRef.current?.focus();
    };

    const selectChat = (chat: ChatSession) => {
        setCurrentChatId(chat.id);
        setMessages(chat.messages);
        if (chat.providerId && providers.some((p) => p.id === chat.providerId)) {
            setActiveProviderId(chat.providerId);
        }
    };

    const deleteChat = (e: React.MouseEvent, id: number) => {
        e.stopPropagation();
        setHistory((prev) => {
            const next = prev.filter((h) => h.id !== id);
            if (next.length === 0) localStorage.removeItem('lumina_history');
            return next;
        });
        if (currentChatId === id) startNewChat();
    };

    const suggestions = [
        { icon: Cpu, label: 'Проверить систему', prompt: 'Покажи запущенные процессы системы и кратко проанализируй их.' },
        { icon: Code, label: 'Помощь с кодом', prompt: 'Напиши на Python скрипт для разбора JSON-файла.' },
        { icon: Sparkles, label: 'Объяснить', prompt: 'Объясни простыми словами, что такое RAG.' },
        { icon: PenTool, label: 'Текст', prompt: 'Помоги составить письмо команде о переносе встречи.' },
    ];

    return (
        <div className="h-screen w-screen flex flex-col overflow-hidden">
            {/* HEADER */}
            <header
                className="h-14 shrink-0 border-b border-border flex items-center justify-between px-3 z-20"
                data-tauri-drag-region
            >
                <div className="flex items-center gap-2" data-tauri-drag-region>
                    <button
                        onClick={() => setIsSidebarOpen((v) => !v)}
                        className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated transition-colors"
                        title="Боковая панель"
                    >
                        <PanelLeft size={17} />
                    </button>
                    <LuminaLogo className="w-6 h-6" />
                    <span className="font-semibold text-[15px] tracking-tight select-none" data-tauri-drag-region>
                        Lumina
                    </span>
                </div>

                <div className="flex items-center gap-1.5">
                    {activeProvider && (
                        <button
                            onClick={() => setShowSettings(true)}
                            className="flex items-center gap-2 px-2.5 h-8 rounded-lg border border-border hover:bg-elevated transition-colors max-w-[240px]"
                            title="Сменить провайдера и модель"
                        >
                            <span className="w-1.5 h-1.5 rounded-full bg-accent shrink-0" />
                            <span className="text-xs text-muted truncate">{activeProvider.label}</span>
                            <span className="text-xs font-mono text-fg truncate">{activeProvider.model}</span>
                        </button>
                    )}
                    <button
                        onClick={toggleTheme}
                        className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated transition-colors"
                        title="Тема"
                    >
                        {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
                    </button>
                    <button
                        onClick={() => setShowPlugins(true)}
                        className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated transition-colors"
                        title="Плагины"
                    >
                        <Blocks size={16} />
                    </button>
                    <button
                        onClick={() => setShowSettings(true)}
                        className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated transition-colors"
                        title="Настройки"
                    >
                        <Settings size={16} />
                    </button>
                    <div className="w-px h-4 bg-border mx-1" />
                    <WindowControls />
                </div>
            </header>

            <div className="flex-1 flex overflow-hidden">
                {/* SIDEBAR */}
                {isSidebarOpen && (
                    <aside className="w-64 shrink-0 border-r border-border flex flex-col bg-bg/20">
                        <div className="p-3">
                            <button
                                onClick={startNewChat}
                                className="w-full h-10 rounded-lg border border-border hover:bg-elevated text-sm font-medium flex items-center justify-center gap-2 transition-colors group"
                            >
                                <Plus size={16} className="group-hover:rotate-90 transition-transform" />
                                Новый чат
                            </button>
                        </div>
                        <div className="flex-1 overflow-y-auto custom-scrollbar px-2 pb-2">
                            <div className="text-[11px] font-semibold text-subtle uppercase tracking-wider px-2 py-2">
                                История
                            </div>
                            {history.length === 0 && (
                                <div className="px-2 text-xs text-subtle">Пока пусто.</div>
                            )}
                            {history.map((h) => (
                                <div key={h.id} className="group relative">
                                    <button
                                        onClick={() => selectChat(h)}
                                        className={`w-full text-left pl-3 pr-8 py-2 rounded-lg text-sm truncate flex items-center gap-2.5 transition-colors ${
                                            currentChatId === h.id
                                                ? 'bg-elevated text-fg'
                                                : 'text-muted hover:bg-elevated/60 hover:text-fg'
                                        }`}
                                    >
                                        <MessageSquare size={14} className="shrink-0 opacity-60" />
                                        <span className="truncate">{h.title}</span>
                                    </button>
                                    <button
                                        onClick={(e) => deleteChat(e, h.id)}
                                        className="absolute right-1.5 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 w-6 h-6 flex items-center justify-center rounded-md text-subtle hover:text-red-400 hover:bg-red-500/10 transition-all"
                                    >
                                        <Trash2 size={13} />
                                    </button>
                                </div>
                            ))}
                        </div>
                    </aside>
                )}

                {/* CHAT */}
                <main className="flex-1 flex flex-col min-w-0">
                    <div className="flex-1 overflow-y-auto custom-scrollbar">
                        {messages.length === 0 ? (
                            <div className="h-full flex flex-col items-center justify-center px-6 select-none">
                                <LuminaLogo className="w-14 h-14 mb-5" />
                                <h1 className="text-2xl font-semibold tracking-tight mb-1">Чем помочь?</h1>
                                <p className="text-sm text-subtle mb-8">
                                    {activeProvider ? `${activeProvider.label} · ${activeProvider.model}` : 'Настройте провайдера'}
                                </p>
                                <div className="grid grid-cols-2 gap-2.5 max-w-xl w-full">
                                    {suggestions.map((s) => (
                                        <button
                                            key={s.label}
                                            onClick={() => handleSend(s.prompt)}
                                            className="p-3.5 rounded-xl border border-border hover:bg-elevated text-left transition-colors group"
                                        >
                                            <s.icon size={17} className="text-accent mb-2" />
                                            <div className="text-sm font-medium">{s.label}</div>
                                            <div className="text-xs text-subtle truncate mt-0.5">{s.prompt}</div>
                                        </button>
                                    ))}
                                </div>
                            </div>
                        ) : (
                            <div className="max-w-3xl mx-auto px-5 py-6 space-y-6">
                                {messages.map((m, i) => {
                                    const isLast = i === messages.length - 1;
                                    const streaming = isLoading && isLast && m.role === 'assistant';
                                    return <ChatMessage key={i} message={m} streaming={streaming} />;
                                })}
                                {isLoading && statusMsg && (
                                    <div className="flex items-center gap-2.5 pl-[42px] text-xs text-accent">
                                        <span className="w-3.5 h-3.5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
                                        {statusMsg}
                                    </div>
                                )}
                                {isLoading &&
                                    messages[messages.length - 1]?.role === 'assistant' &&
                                    !messages[messages.length - 1]?.content &&
                                    !statusMsg && (
                                        <div className="pl-[42px]">
                                            <TypingIndicator />
                                        </div>
                                    )}
                                <div ref={messagesEndRef} />
                            </div>
                        )}
                    </div>

                    {/* INPUT */}
                    <div className="px-5 pb-5 pt-1">
                        <div className="max-w-3xl mx-auto">
                            {attachments.length > 0 && (
                                <div className="flex flex-wrap gap-2 mb-2">
                                    {attachments.map((file, i) => (
                                        <div
                                            key={i}
                                            className="flex items-center gap-1.5 pl-2 pr-1 py-1 rounded-md bg-elevated border border-border text-xs"
                                        >
                                            <FileText size={12} className="text-accent" />
                                            <span className="truncate max-w-[160px]">{file.split(/[\\/]/).pop()}</span>
                                            <button
                                                onClick={() => setAttachments((prev) => prev.filter((_, idx) => idx !== i))}
                                                className="text-subtle hover:text-red-400"
                                            >
                                                <X size={12} />
                                            </button>
                                        </div>
                                    ))}
                                </div>
                            )}
                            <div className="flex items-end gap-2 bg-surface border border-border rounded-xl p-2 focus-within:border-accent transition-colors">
                                <button
                                    onClick={handleAttach}
                                    className={`w-9 h-9 shrink-0 flex items-center justify-center rounded-lg transition-colors ${
                                        attachments.length > 0 ? 'text-accent' : 'text-subtle hover:text-fg hover:bg-elevated'
                                    }`}
                                    title="Прикрепить файлы"
                                >
                                    <Paperclip size={17} />
                                </button>
                                <textarea
                                    ref={inputRef}
                                    value={input}
                                    onChange={(e) => setInput(e.target.value)}
                                    onKeyDown={(e) => {
                                        if (e.key === 'Enter' && !e.shiftKey) {
                                            e.preventDefault();
                                            handleSend();
                                        }
                                    }}
                                    rows={1}
                                    disabled={isLoading}
                                    placeholder={isLoading ? 'Lumina печатает…' : 'Спросите Lumina… (Enter — отправить, Shift+Enter — новая строка)'}
                                    className="flex-1 bg-transparent resize-none outline-none text-sm py-2 max-h-40 placeholder:text-subtle disabled:opacity-60"
                                    style={{ minHeight: '20px' }}
                                />
                                <button
                                    onClick={() => handleSend()}
                                    disabled={isLoading || (!input.trim() && attachments.length === 0)}
                                    className="w-9 h-9 shrink-0 flex items-center justify-center rounded-lg bg-accent text-accent-fg disabled:bg-elevated disabled:text-subtle transition-colors"
                                >
                                    <Send size={16} />
                                </button>
                            </div>
                            <p className="text-[11px] text-center mt-2 text-subtle select-none">
                                Работает через ваш провайдер — ключи хранятся локально.
                            </p>
                        </div>
                    </div>
                </main>
            </div>

            {/* MODALS */}
            {showSettings && activeProvider && (
                <ProviderSettings
                    providers={providers}
                    activeId={activeProviderId}
                    store={store}
                    useGraph={useGraph}
                    onChange={setProviders}
                    onStoreChange={setStore}
                    onUseGraphChange={setUseGraph}
                    onSelect={setActiveProviderId}
                    onClose={() => setShowSettings(false)}
                />
            )}
            {showPlugins && <PluginsModal onClose={() => setShowPlugins(false)} />}
        </div>
    );
};

// --- ПЛАГИНЫ (QuickJS) ---
const PluginsModal: React.FC<{ onClose: () => void }> = ({ onClose }) => {
    const [code, setCode] = useState('print("Привет из QuickJS!");\n1 + 2;');
    const [result, setResult] = useState('');
    const [running, setRunning] = useState(false);

    const run = async () => {
        setRunning(true);
        try {
            setResult(await pluginService.runPlugin(code));
        } finally {
            setRunning(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-center justify-center p-6" onClick={onClose}>
            <div
                className="bg-surface border border-border rounded-2xl w-full max-w-2xl h-[520px] shadow-2xl flex flex-col overflow-hidden"
                onClick={(e) => e.stopPropagation()}
            >
                <div className="px-5 h-14 flex items-center justify-between border-b border-border">
                    <div className="flex items-center gap-2 text-sm font-semibold">
                        <Blocks size={17} className="text-accent" /> Плагины (QuickJS)
                    </div>
                    <button onClick={onClose} className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated">
                        <X size={18} />
                    </button>
                </div>
                <div className="flex-1 flex flex-col gap-3 p-5 min-h-0">
                    <textarea
                        value={code}
                        onChange={(e) => setCode(e.target.value)}
                        spellCheck={false}
                        className="flex-1 bg-bg/50 border border-border rounded-xl p-4 text-sm font-mono resize-none outline-none focus:border-accent transition-colors custom-scrollbar"
                    />
                    {result && (
                        <div className="bg-bg/50 border border-border rounded-xl p-4">
                            <div className="text-[11px] font-semibold text-subtle uppercase tracking-wider mb-1">Результат</div>
                            <pre className="text-sm font-mono whitespace-pre-wrap">{result}</pre>
                        </div>
                    )}
                    <button
                        onClick={run}
                        disabled={running}
                        className="h-10 rounded-lg bg-accent text-accent-fg font-medium text-sm flex items-center justify-center gap-2 hover:opacity-90 disabled:opacity-60 transition-opacity"
                    >
                        <Play size={15} /> {running ? 'Выполняю…' : 'Запустить'}
                    </button>
                </div>
            </div>
        </div>
    );
};

export default App;
