import { readDir, readTextFile, writeTextFile, mkdir, exists } from '@tauri-apps/plugin-fs';

export const fsService = {
    async listDir(path: string) {
        try {
            return await readDir(path);
        } catch (e) {
            console.error('FS Error:', e);
            throw e;
        }
    },

    async readFile(path: string) {
        return await readTextFile(path);
    },

    async writeFile(path: string, content: string) {
        return await writeTextFile(path, content);
    },

    async createDir(path: string) {
        if (!await exists(path)) {
            return await mkdir(path, { recursive: true });
        }
    },

    async exists(path: string) {
        return await exists(path);
    }
};
