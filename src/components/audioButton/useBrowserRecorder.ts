import { useRef, useState } from "react";
import { writeFile, BaseDirectory, mkdir } from '@tauri-apps/plugin-fs';

export function useBrowserRecorder() {
    const [recording, setRecording] = useState(false);

    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const chunksRef = useRef<Blob[]>([]);

    const start = async () => {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

        const recorder = new MediaRecorder(stream);
        mediaRecorderRef.current = recorder;
        chunksRef.current = [];

        recorder.ondataavailable = (e) => {
            if (e.data.size > 0) {
                chunksRef.current.push(e.data);
            }
        };

        recorder.start();
        setRecording(true);
    };

    const stop = async (): Promise<Blob> => {
        return new Promise((resolve) => {
            const recorder = mediaRecorderRef.current;
            if (!recorder) return;

            recorder.onstop = () => {
                const blob = new Blob(chunksRef.current, { type: "audio/wav" });
                setRecording(false);
                resolve(blob);
            };

            recorder.stop();
        });
    };


    const saveAudio = async (blob: Blob) => {
        const buffer = await blob.arrayBuffer();
        const uint8 = new Uint8Array(buffer);

        const fileName = `recording-${Date.now()}.wav`;
console.log("Saving audio as: ", fileName);
    // 1. Ensure the AppData directory exists first
    await mkdir('voice-recordings', { 
        baseDir: BaseDirectory.AppData,
        recursive: true 
    });
    
        await writeFile('voice-recordings/' + fileName, uint8, {
            baseDir: BaseDirectory.AppData,
        });
console.log("audio saved: ", fileName);
        return fileName;
    };


    return { start, stop, recording, saveAudio };
}