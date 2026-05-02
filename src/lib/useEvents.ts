import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useEffect } from 'react';
import { useAppStore } from './store';

export default function useEvents() {
    const addMessage      = useAppStore((s) => s.addMessage);
    const appendToken     = useAppStore((s) => s.appendToken);
    const finalizeStream  = useAppStore((s) => s.finalizeStream);
    const setSessionStatus = useAppStore((s) => s.setSessionStatus);

    useEffect(() => {
        const cleanup: UnlistenFn[] = [];

        async function register() {
            cleanup.push(
                await listen<{ text: string }>('transcript', (e) =>
                    addMessage('user', e.payload.text)
                ),
                await listen<{ token: string }>('response_token', (e) =>
                    appendToken(e.payload.token)
                ),
                await listen<{ full_response: string; milestone: boolean }>('response_done', (e) =>
                    finalizeStream(e.payload.full_response)
                ),
                await listen<{ greeting: string }>('session_ready', () =>
                    setSessionStatus('ready')
                ),
                await listen<{ message: string }>('error', (e) => {
                    console.error('[backend]', e.payload.message);
                    addMessage('system', `Error: ${e.payload.message}`);
                    setSessionStatus('idle');
                }),
            );
        }

        register();
        return () => cleanup.forEach((fn) => fn());
    }, [addMessage, appendToken, finalizeStream, setSessionStatus]);
}
