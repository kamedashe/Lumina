import { ProviderConfig, ProviderKind } from './types';

export const SYSTEM_PROMPT = `You are Lumina, an intelligent desktop assistant running on the user's machine.

You have access to tools for working with the local system:
- list_files / read_file / write_file — inspect and edit files
- get_current_dir — the current working directory
- list_processes — running system processes
- search_documents — semantic search over documents the user has attached

Use a tool when it genuinely helps answer the request; otherwise just answer directly. Do not ask for permission before using a read-only tool. When a tool returns data, present it cleanly with Markdown (code blocks for commands and file contents, tables for structured data).

Reply in the same language the user writes in. Be concise and lead with the answer.`;

/** Заготовки провайдеров: пользователь выбирает тип и дописывает ключ/модель. */
export type ProviderPreset = {
    kind: ProviderKind;
    label: string;
    base_url: string;
    needsKey: boolean;
    hint: string;
    defaultModel: string;
    embedding_model?: string;
};

export const PROVIDER_PRESETS: Record<string, ProviderPreset> = {
    ollama: {
        kind: 'ollama',
        label: 'Ollama (локально)',
        base_url: 'http://localhost:11434',
        needsKey: false,
        hint: 'Локальные модели. Ключ не нужен.',
        defaultModel: 'llama3',
        embedding_model: 'nomic-embed-text',
    },
    openai: {
        kind: 'open_ai',
        label: 'OpenAI',
        base_url: 'https://api.openai.com/v1',
        needsKey: true,
        hint: 'Ключ вида sk-…',
        defaultModel: 'gpt-4o-mini',
        embedding_model: 'text-embedding-3-small',
    },
    openrouter: {
        kind: 'open_ai',
        label: 'OpenRouter',
        base_url: 'https://openrouter.ai/api/v1',
        needsKey: true,
        hint: 'Единый ключ к сотням моделей.',
        defaultModel: 'openai/gpt-4o-mini',
    },
    groq: {
        kind: 'open_ai',
        label: 'Groq',
        base_url: 'https://api.groq.com/openai/v1',
        needsKey: true,
        hint: 'Очень быстрый инференс.',
        defaultModel: 'llama-3.3-70b-versatile',
    },
    lmstudio: {
        kind: 'open_ai',
        label: 'LM Studio (локально)',
        base_url: 'http://localhost:1234/v1',
        needsKey: false,
        hint: 'Локальный OpenAI-совместимый сервер.',
        defaultModel: 'local-model',
    },
    anthropic: {
        kind: 'anthropic',
        label: 'Anthropic (Claude)',
        base_url: 'https://api.anthropic.com',
        needsKey: true,
        hint: 'Ключ вида sk-ant-…',
        defaultModel: 'claude-opus-4-8',
    },
    gemini: {
        kind: 'gemini',
        label: 'Google Gemini',
        base_url: 'https://generativelanguage.googleapis.com',
        needsKey: true,
        hint: 'Ключ из Google AI Studio.',
        defaultModel: 'gemini-2.5-flash',
        embedding_model: 'text-embedding-004',
    },
};

export function makeProviderId(): string {
    return `p_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
}

/**
 * Чем считать эмбеддинги для RAG. У Anthropic эндпоинта эмбеддингов нет вообще,
 * поэтому для Claude берём настроенного пользователем Ollama — с его base_url,
 * а не с захардкоженного localhost. Для остальных провайдеров undefined:
 * бэкенд посчитает эмбеддинги самим активным провайдером.
 */
export function resolveEmbeddingProvider(
    active: ProviderConfig,
    all: ProviderConfig[],
): ProviderConfig | undefined {
    if (active.kind !== 'anthropic') return undefined;

    const ollama = all.find((p) => p.kind === 'ollama');
    if (!ollama) return undefined;

    return {
        ...ollama,
        embedding_model: ollama.embedding_model || 'nomic-embed-text',
    };
}

/** Провайдер по умолчанию — локальный Ollama, чтобы приложение работало из коробки. */
export function defaultProvider(): ProviderConfig {
    const preset = PROVIDER_PRESETS.ollama;
    return {
        id: makeProviderId(),
        kind: preset.kind,
        label: preset.label,
        base_url: preset.base_url,
        api_key: '',
        model: preset.defaultModel,
        temperature: 0.7,
        embedding_model: preset.embedding_model,
    };
}
