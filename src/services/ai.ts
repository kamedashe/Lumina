import { invoke, Channel } from '@tauri-apps/api/core';
import { ProviderConfig, StreamEvent, WireMessage } from '../types';

export const aiService = {
    /**
     * Запускает агентный цикл. Вся история диалога уходит на бэкенд, ответ
     * стримится обратно через Channel по мере генерации.
     */
    async runAgent(params: {
        config: ProviderConfig;
        system: string;
        messages: WireMessage[];
        attachments: string[];
        onEvent: (event: StreamEvent) => void;
    }): Promise<void> {
        const channel = new Channel<StreamEvent>();
        channel.onmessage = params.onEvent;

        await invoke('run_agent', {
            config: params.config,
            system: params.system,
            messages: params.messages,
            attachments: params.attachments,
            onEvent: channel,
        });
    },

    async listModels(config: ProviderConfig): Promise<string[]> {
        try {
            return await invoke<string[]>('list_models', { config });
        } catch (e) {
            console.error('listModels failed:', e);
            throw e;
        }
    },

    async testProvider(config: ProviderConfig): Promise<boolean> {
        return await invoke<boolean>('test_provider', { config });
    },
};
