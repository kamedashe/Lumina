import React, { useState } from 'react';
import { HardDrive, Cloud, Zap, CheckCircle2, XCircle, RefreshCw, KeyRound, ShieldAlert } from 'lucide-react';
import { VectorStoreConfig, VectorStoreKind } from '../types';
import { aiService } from '../services/ai';

type Props = {
    store: VectorStoreConfig;
    onChange: (store: VectorStoreConfig) => void;
};

type TestState = 'idle' | 'testing' | 'ok' | 'fail';

export const VectorStoreSettings: React.FC<Props> = ({ store, onChange }) => {
    const [testState, setTestState] = useState<TestState>('idle');
    const [testMsg, setTestMsg] = useState('');
    const [showKey, setShowKey] = useState(false);

    const update = (patch: Partial<VectorStoreConfig>) => onChange({ ...store, ...patch });

    const test = async () => {
        setTestState('testing');
        setTestMsg('');
        try {
            setTestMsg(await aiService.testVectorStore(store));
            setTestState('ok');
        } catch (e) {
            setTestMsg(String(e));
            setTestState('fail');
        }
    };

    const options: { kind: VectorStoreKind; icon: typeof HardDrive; title: string; note: string }[] = [
        { kind: 'sqlite_vec', icon: Zap, title: 'sqlite-vec', note: 'Локально, быстрый перебор' },
        { kind: 'sqlite', icon: HardDrive, title: 'SQLite', note: 'Локально, без расширений' },
        { kind: 'pinecone', icon: Cloud, title: 'Pinecone', note: 'Облако, ANN-индекс' },
    ];

    return (
        <div className="space-y-5">
            <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted">Векторное хранилище</label>
                <div className="grid grid-cols-3 gap-2">
                    {options.map((o) => (
                        <button
                            key={o.kind}
                            onClick={() => {
                                update({ kind: o.kind });
                                setTestState('idle');
                                setTestMsg('');
                            }}
                            className={`p-3 rounded-lg border text-left transition-colors ${
                                store.kind === o.kind
                                    ? 'border-accent bg-accent/10'
                                    : 'border-border hover:bg-elevated'
                            }`}
                        >
                            <o.icon size={16} className={store.kind === o.kind ? 'text-accent' : 'text-muted'} />
                            <div className="text-sm font-medium mt-1.5">{o.title}</div>
                            <div className="text-[11px] text-subtle">{o.note}</div>
                        </button>
                    ))}
                </div>
            </div>

            {store.kind === 'sqlite_vec' && (
                <p className="text-[11px] leading-relaxed text-subtle">
                    Векторы лежат бинарными блобами, расстояния считает SIMD-код расширения.
                    Это <b>не ANN</b> — перебор остаётся полным, но во много раз дешевле, чем
                    разбирать JSON на каждой строке. При смене модели эмбеддингов индекс
                    пересоздаётся автоматически: документы нужно прикрепить заново.
                </p>
            )}

            {store.kind === 'sqlite' && (
                <p className="text-[11px] leading-relaxed text-subtle">
                    Запасной вариант без сторонних расширений: эмбеддинги хранятся как JSON и
                    разбираются на каждый запрос. Работает везде, но заметно медленнее sqlite-vec.
                </p>
            )}

            {store.kind === 'pinecone' && (
                <>
                    <div className="flex gap-2 p-3 rounded-lg border border-amber-500/30 bg-amber-500/10">
                        <ShieldAlert size={15} className="text-amber-500 shrink-0 mt-0.5" />
                        <p className="text-[11px] leading-relaxed text-muted">
                            В Pinecone уходят не только векторы, но и <b>текст фрагментов</b> — он хранится
                            в метаданных, иначе его нечего вернуть модели в контекст. Не используйте облачный
                            индекс для конфиденциальных документов.
                        </p>
                    </div>

                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-muted">API-ключ Pinecone</label>
                        <div className="relative">
                            <KeyRound size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-subtle" />
                            <input
                                type={showKey ? 'text' : 'password'}
                                value={store.api_key ?? ''}
                                onChange={(e) => update({ api_key: e.target.value })}
                                placeholder="pcsk_…"
                                spellCheck={false}
                                className="w-full bg-bg/50 border border-border rounded-lg pl-9 pr-16 py-2.5 text-xs font-mono outline-none focus:border-accent transition-colors"
                            />
                            <button
                                onClick={() => setShowKey((s) => !s)}
                                className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-subtle hover:text-fg"
                            >
                                {showKey ? 'скрыть' : 'показать'}
                            </button>
                        </div>
                    </div>

                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-muted">Имя индекса</label>
                        <input
                            value={store.index_name ?? ''}
                            onChange={(e) => update({ index_name: e.target.value })}
                            placeholder="lumina-docs"
                            spellCheck={false}
                            className="w-full bg-bg/50 border border-border rounded-lg px-3 py-2.5 text-xs font-mono outline-none focus:border-accent transition-colors"
                        />
                        <p className="text-[11px] text-subtle">
                            Хост индекса определяется автоматически. Размерность индекса должна совпадать
                            с моделью эмбеддингов: nomic-embed-text — 768, text-embedding-3-small — 1536.
                        </p>
                    </div>

                    <div className="space-y-1.5">
                        <label className="text-xs font-medium text-muted">Namespace (необязательно)</label>
                        <input
                            value={store.namespace ?? ''}
                            onChange={(e) => update({ namespace: e.target.value })}
                            placeholder="default"
                            spellCheck={false}
                            className="w-full bg-bg/50 border border-border rounded-lg px-3 py-2.5 text-xs font-mono outline-none focus:border-accent transition-colors"
                        />
                    </div>
                </>
            )}

            <button
                onClick={test}
                disabled={testState === 'testing'}
                className="px-3 py-2 rounded-lg border border-border text-muted hover:text-fg hover:bg-elevated flex items-center gap-1.5 text-xs transition-colors disabled:opacity-50"
            >
                <RefreshCw size={13} className={testState === 'testing' ? 'animate-spin' : ''} />
                Проверить хранилище
            </button>

            {testState !== 'idle' && testState !== 'testing' && (
                <div className={`flex items-start gap-2 text-xs ${testState === 'ok' ? 'text-accent' : 'text-red-400'}`}>
                    {testState === 'ok' ? (
                        <CheckCircle2 size={14} className="shrink-0 mt-0.5" />
                    ) : (
                        <XCircle size={14} className="shrink-0 mt-0.5" />
                    )}
                    <span className="break-words">{testMsg}</span>
                </div>
            )}
        </div>
    );
};
