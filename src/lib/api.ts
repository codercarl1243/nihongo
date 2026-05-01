import {invoke} from "@tauri-apps/api/core";

export async function transcribeAudio(path: string): Promise<string> {
    return await invoke("transcribe_audio_cmd", { 
        path
     });
}