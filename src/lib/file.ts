import { appDataDir } from '@tauri-apps/api/path';
import { join } from '@tauri-apps/api/path';

export async function resolvePath(fileName: string): Promise<string> {
    const dir = await appDataDir();
    const fullPath = await join(dir, "voice-recordings", fileName);
    return fullPath;
}