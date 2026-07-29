import React, { useState } from 'react';
import { X, Plus, Trash2, RefreshCw, CheckCircle2, XCircle, Server, Cloud, KeyRound, Database } from 'lucide-react';
import { ProviderConfig, VectorStoreConfig } from '../types';
import { PROVIDER_PRESETS, makeProviderId } from '../constants';
import { aiService } from '../services/ai';
import { VectorStoreSettings } from './VectorStoreSettings';

type TestState = 'idle' | 'testing' | 'ok' | 'fail';

type Props = {
    providers: ProviderConfig[];
    activeId: string;
    store: VectorStoreConfig;
    useGraph: boolean;
    onChange: (providers: ProviderConfig[]) => void;
    onStoreChange: (store: VectorStoreConfig) => void;
    onUseGraphChange: (value: boolean) => void;
    onSelect: (id: string) => void;
    onClose: () => void;
};

export const ProviderSettings: React.FC<Props> = ({
    providers,
    activeId,
    store,
    useGraph,
    onChange,
    onStoreChange,
    onUseGraphChange,
    onSelect,
    onClose,
}) => {
    const [editingId, setEditingId] = useState<string | null>(activeId);
    const [view, setView] = useState<'provider' | 'store'>('provider');
    const [models, setModels] = useState<string[]>([]);
    const [testState, setTestState] = useState<TestState>('idle');
    const [testMsg, setTestMsg] = useState<string>('');
    const [showKey, setShowKey] = useState(false);

    const editing = providers.find((p) => p.id === editingId) ?? null;

    const update = (patch: Partial<ProviderConfig>) => {
        if (!editing) return;
        onChange(providers.map((p) => (p.id === editing.id ? { ...p, ...patch } : p)));
    };

    const addFromPreset = (presetKey: string) => {
        const preset = PROVIDER_PRESETS[presetKey];
        const provider: ProviderConfig = {
            id: makeProviderId(),
            kind: preset.kind,
            label: preset.label,
            base_url: preset.base_url,
            api_key: '',
            model: preset.defaultModel,
            temperature: 0.7,
            embedding_model: preset.embedding_model,
        };
        onChange([...providers, provider]);
        setEditingId(provider.id);
        setModels([]);
        setTestState('idle');
    };

    const removeProvider = (id: string) => {
        const next = providers.filter((p) => p.id !== id);
        onChange(next);
        if (activeId === id && next[0]) onSelect(next[0].id);
        if (editingId === id) setEditingId(next[0]?.id ?? null);
    };

    const fetchModels = async () => {
        if (!editing) return;
        setTestState('testing');
        setTestMsg('');
        try {
            const list = await aiService.listModels(editing);
            setModels(list);
            setTestState('ok');
            setTestMsg(`Найдено моделей: ${list.length}`);
            if (list.length && !list.includes(editing.model)) {
                update({ model: list[0] });
            }
        } catch (e) {
            setTestState('fail');
            setTestMsg(String(e));
            setModels([]);
        }
    };

    return (
        <div
            className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-center justify-center p-6"
            onClick={onClose}
        >
            <div
                className="bg-surface border border-border rounded-2xl w-full max-w-3xl h-[560px] shadow-2xl flex overflow-hidden"
                onClick={(e) => e.stopPropagation()}
            >
                {/* Левая колонка: список провайдеров */}
                <div className="w-56 shrink-0 border-r border-border flex flex-col bg-bg/30">
                    <div className="px-4 h-14 flex items-center border-b border-border">
                        <span className="text-sm font-semibold">Провайдеры</span>
                    </div>
                    <div className="flex-1 overflow-y-auto custom-scrollbar p-2 space-y-1">
                        {providers.map((p) => (
                            <button
                                key={p.id}
                                onClick={() => {
                                    setView('provider');
                                    setEditingId(p.id);
                                    setModels([]);
                                    setTestState('idle');
                                    setTestMsg('');
                                }}
                                className={`w-full text-left px-3 py-2 rounded-lg text-sm flex items-center gap-2 transition-colors ${
                                    view === 'provider' && editingId === p.id
                                        ? 'bg-elevated text-fg'
                                        : 'text-muted hover:bg-elevated/60 hover:text-fg'
                                }`}
                            >
                                {p.kind === 'ollama' ? <Server size={14} /> : <Cloud size={14} />}
                                <span className="truncate flex-1">{p.label}</span>
                                {activeId === p.id && <span className="w-1.5 h-1.5 rounded-full bg-accent" />}
                            </button>
                        ))}

                        <div className="pt-3 mt-2 border-t border-border">
                            <button
                                onClick={() => setView('store')}
                                className={`w-full text-left px-3 py-2 rounded-lg text-sm flex items-center gap-2 transition-colors ${
                                    view === 'store'
                                        ? 'bg-elevated text-fg'
                                        : 'text-muted hover:bg-elevated/60 hover:text-fg'
                                }`}
                            >
                                <Database size={14} />
                                <span className="truncate flex-1">Хранилище (RAG)</span>
                                <span className="text-[10px] text-subtle uppercase">{store.kind}</span>
                            </button>
                        </div>
                    </div>
                    <div className="p-2 border-t border-border">
                        <details className="group">
                            <summary className="list-none cursor-pointer px-3 py-2 rounded-lg text-sm text-muted hover:text-fg hover:bg-elevated/60 flex items-center gap-2">
                                <Plus size={14} /> Добавить провайдера
                            </summary>
                            <div className="mt-1 space-y-0.5">
                                {Object.entries(PROVIDER_PRESETS).map(([key, preset]) => (
                                    <button
                                        key={key}
                                        onClick={() => addFromPreset(key)}
                                        className="w-full text-left px-3 py-1.5 rounded-md text-xs text-muted hover:text-fg hover:bg-elevated transition-colors"
                                    >
                                        {preset.label}
                                    </button>
                                ))}
                            </div>
                        </details>
                    </div>
                </div>

                {/* Правая колонка: редактор выбранного провайдера */}
                <div className="flex-1 flex flex-col min-w-0">
                    <div className="px-5 h-14 flex items-center justify-between border-b border-border">
                        <span className="text-sm font-semibold">
                            {view === 'store' ? 'Векторное хранилище' : 'Настройки провайдера'}
                        </span>
                        <button
                            onClick={onClose}
                            className="w-8 h-8 flex items-center justify-center rounded-md text-muted hover:text-fg hover:bg-elevated"
                        >
                            <X size={18} />
                        </button>
                    </div>

                    {view === 'store' ? (
                        <div className="flex-1 overflow-y-auto custom-scrollbar p-5">
                            <VectorStoreSettings
                                store={store}
                                onChange={onStoreChange}
                                useGraph={useGraph}
                                onUseGraphChange={onUseGraphChange}
                            />
                        </div>
                    ) : !editing ? (
                        <div className="flex-1 flex items-center justify-center text-sm text-subtle">
                            Добавьте провайдера слева, чтобы начать.
                        </div>
                    ) : (
                        <div className="flex-1 overflow-y-auto custom-scrollbar p-5 space-y-5">
                            <Field label="Название">
                                <input
                                    value={editing.label}
                                    onChange={(e) => update({ label: e.target.value })}
                                    className="input"
                                />
                            </Field>

                            <Field label="Base URL">
                                <input
                                    value={editing.base_url}
                                    onChange={(e) => update({ base_url: e.target.value })}
                                    className="input font-mono text-xs"
                                    spellCheck={false}
                                />
                            </Field>

                            {editing.kind !== 'ollama' && (
                                <Field label="API-ключ">
                                    <div className="relative">
                                        <KeyRound
                                            size={14}
                                            className="absolute left-3 top-1/2 -translate-y-1/2 text-subtle"
                                        />
                                        <input
                                            type={showKey ? 'text' : 'password'}
                                            value={editing.api_key ?? ''}
                                            onChange={(e) => update({ api_key: e.target.value })}
                                            placeholder="sk-…"
                                            className="input pl-9 pr-16 font-mono text-xs"
                                            spellCheck={false}
                                        />
                                        <button
                                            onClick={() => setShowKey((s) => !s)}
                                            className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-subtle hover:text-fg"
                                        >
                                            {showKey ? 'скрыть' : 'показать'}
                                        </button>
                                    </div>
                                </Field>
                            )}

                            <Field label="Модель">
                                <div className="flex gap-2">
                                    {models.length > 0 ? (
                                        <select
                                            value={editing.model}
                                            onChange={(e) => update({ model: e.target.value })}
                                            className="input flex-1"
                                        >
                                            {models.map((m) => (
                                                <option key={m} value={m}>
                                                    {m}
                                                </option>
                                            ))}
                                        </select>
                                    ) : (
                                        <input
                                            value={editing.model}
                                            onChange={(e) => update({ model: e.target.value })}
                                            className="input flex-1 font-mono text-xs"
                                            spellCheck={false}
                                        />
                                    )}
                                    <button
                                        onClick={fetchModels}
                                        disabled={testState === 'testing'}
                                        className="shrink-0 px-3 rounded-lg border border-border text-muted hover:text-fg hover:bg-elevated flex items-center gap-1.5 text-xs transition-colors disabled:opacity-50"
                                    >
                                        <RefreshCw
                                            size={13}
                                            className={testState === 'testing' ? 'animate-spin' : ''}
                                        />
                                        Модели
                                    </button>
                                </div>
                            </Field>

                            {testState !== 'idle' && testState !== 'testing' && (
                                <div
                                    className={`flex items-center gap-2 text-xs ${
                                        testState === 'ok' ? 'text-accent' : 'text-red-400'
                                    }`}
                                >
                                    {testState === 'ok' ? (
                                        <CheckCircle2 size={14} />
                                    ) : (
                                        <XCircle size={14} />
                                    )}
                                    <span className="break-words">{testMsg}</span>
                                </div>
                            )}

                            <Field label={`Температура — ${editing.temperature.toFixed(1)}`}>
                                <input
                                    type="range"
                                    min={0}
                                    max={1}
                                    step={0.1}
                                    value={editing.temperature}
                                    onChange={(e) => update({ temperature: parseFloat(e.target.value) })}
                                    className="w-full accent-accent"
                                />
                            </Field>

                            <div className="pt-2 flex items-center justify-between">
                                <button
                                    onClick={() => {
                                        onSelect(editing.id);
                                        onClose();
                                    }}
                                    className="px-4 py-2 rounded-lg bg-accent text-accent-fg text-sm font-medium hover:opacity-90 transition-opacity"
                                >
                                    Использовать этот провайдер
                                </button>
                                {providers.length > 1 && (
                                    <button
                                        onClick={() => removeProvider(editing.id)}
                                        className="px-3 py-2 rounded-lg text-sm text-muted hover:text-red-400 hover:bg-red-500/10 flex items-center gap-1.5 transition-colors"
                                    >
                                        <Trash2 size={14} /> Удалить
                                    </button>
                                )}
                            </div>
                        </div>
                    )}
                </div>
            </div>

            <style>{`
                .input {
                    width: 100%;
                    background: rgb(var(--bg) / 0.5);
                    border: 1px solid rgb(var(--border));
                    border-radius: 0.6rem;
                    padding: 0.55rem 0.75rem;
                    font-size: 0.875rem;
                    color: rgb(var(--fg));
                    outline: none;
                    transition: border-color 0.15s;
                }
                .input:focus { border-color: rgb(var(--accent)); }
            `}</style>
        </div>
    );
};

const Field: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
    <div className="space-y-1.5">
        <label className="text-xs font-medium text-muted">{label}</label>
        {children}
    </div>
);
