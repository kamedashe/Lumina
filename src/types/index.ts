export type Message = {
    role: 'user' | 'assistant';
    content: string
};

export type ChatSession = {
    id: number;
    title: string;
    date: number;
    messages: Message[]
};

export type OllamaModel = {
    name: string;
};
