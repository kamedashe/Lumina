import { invoke } from '@tauri-apps/api/core';

export const aiService = {
    async getModels(): Promise<string[]> {
        try {
            return await invoke<string[]>('get_ollama_models');
        } catch (e) {
            console.error('Failed to fetch models:', e);
            return [];
        }
    },

    async chat(prompt: string, model: string, temperature: number): Promise<string> {
        return await invoke<string>('chat_with_ollama', {
            model,
            prompt,
            temperature
        });
    },

    async generateTitle(firstMessage: string, model: string): Promise<string> {
        try {
            const title = await invoke<string>('chat_with_ollama', {
                model,
                prompt: `Generate a very short title (max 4 words) for a chat that starts with this message: "${firstMessage}". Do not use quotes. Just the title.`,
                temperature: 0.3
            });
            return title.trim();
        } catch (e) {
            return firstMessage.slice(0, 20) + "...";
        }
    },



    async processDocuments(paths: string[]): Promise<string> {
        return await invoke<string>('process_documents', { paths });
    },

    async searchDocuments(query: string): Promise<string> {
        return await invoke<string>('search_documents', { query });
    }
};
