import { invoke } from '@tauri-apps/api/core';

export const pluginService = {
    async runPlugin(code: string): Promise<string> {
        try {
            return await invoke<string>('run_plugin', { code });
        } catch (e) {
            console.error('Plugin Service Error:', e);
            return `Error: ${e}`;
        }
    }
};
