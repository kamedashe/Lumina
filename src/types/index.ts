export type Role = 'system' | 'user' | 'assistant' | 'tool';

export type ToolCall = {
    id: string;
    name: string;
    arguments: string;
};

export type ToolInvocation = {
    id: string;
    name: string;
    content: string;
    isError: boolean;
};

export type Message = {
    role: 'user' | 'assistant';
    content: string;
    /** Инструменты, выполненные во время генерации этого ответа (для отображения). */
    tools?: ToolInvocation[];
};

/** Формат сообщения, который уходит в бэкенд (совпадает с Rust ChatMessage). */
export type WireMessage = {
    role: Role;
    content: string;
    tool_calls?: ToolCall[];
    tool_call_id?: string;
    tool_name?: string;
};

export type ChatSession = {
    id: number;
    title: string;
    date: number;
    messages: Message[];
    /** Провайдер, которым велась беседа. */
    providerId?: string;
};

export type ProviderKind = 'ollama' | 'open_ai' | 'anthropic' | 'gemini';

export type ProviderConfig = {
    id: string;
    kind: ProviderKind;
    label: string;
    base_url: string;
    api_key?: string;
    model: string;
    temperature: number;
    embedding_model?: string;
};

export type VectorStoreKind = 'sqlite' | 'sqlite_vec' | 'pinecone';

/** Конфигурация векторного хранилища (совпадает с Rust VectorStoreConfig). */
export type VectorStoreConfig = {
    kind: VectorStoreKind;
    api_key?: string;
    index_name?: string;
    namespace?: string;
};

/**
 * Прогонять ли поиск по документам через LangGraph-граф вместо одиночного
 * векторного запроса. Хранится отдельно от VectorStoreConfig: это про то, как
 * ищем, а не про то, где лежат векторы.
 */
export type SearchMode = {
    useGraph: boolean;
};

/** События потока из бэкенда (совпадает с Rust StreamEvent). */
export type StreamEvent =
    | { type: 'text'; delta: string }
    | { type: 'tool_call_start'; id: string; name: string }
    | { type: 'tool_result'; id: string; name: string; content: string; is_error: boolean }
    | { type: 'status'; message: string }
    | { type: 'done'; stop_reason: string }
    | { type: 'error'; message: string };
