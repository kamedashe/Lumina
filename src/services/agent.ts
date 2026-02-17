import { invoke } from '@tauri-apps/api/core';
import { fsService } from './fs';

export const agentService = {
    async generateWorkspaceReport(): Promise<string> {
        try {
            return await invoke<string>('generate_report');
        } catch (e) {
            console.error('Agent Service Error:', e);
            return 'Failed to generate report.';
        }
    },

    async createFile(path: string, content: string): Promise<string> {
        try {
            await fsService.writeFile(path, content);
            return `File created successfully at ${path}`;
        } catch (e) {
            console.error('Agent Service Error:', e);
            return `Failed to create file at ${path}: ${e}`;
        }
    },

    async listFiles(path: string): Promise<string> {
        try {
            const files = await invoke<string[]>('list_dir_contents', { path });
            return files.length > 0 ? files.join('\n') : 'Directory is empty.';
        } catch (e) {
            console.error('Agent Service Error:', e);
            return `Failed to list files in ${path}: ${e}`;
        }
    },

    async getCurrentDir(): Promise<string> {
        try {
            return await invoke<string>('get_current_dir');
        } catch (e) {
            console.error('Agent Service Error:', e);
            return 'Failed to get current directory.';
        }
    },

    async getSystemProcesses(): Promise<string[]> {
        try {
            return await invoke<string[]>('get_system_processes');
        } catch (e) {
            console.error('Agent Service Error:', e);
            return ['Failed to get system processes.'];
        }
    }
};
