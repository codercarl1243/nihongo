import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useEffect } from 'react';
import { useAppStore } from './store';

export default function useEvents() {
    const addMessage       = useAppStore((s) => s.addMessage);
    const appendToken      = useAppStore((s) => s.appendToken);
    const finalizeStream   = useAppStore((s) => s.finalizeStream);
    const setSessionStatus = useAppStore((s) => s.setSessionStatus);

    useEffect(() => {
        // `cancelled` guards against the React StrictMode double-mount pattern:
        // cleanup runs synchronously before the async listen() calls resolve,
        // so we track cancellation and immediately unlisten if we were torn down.
        let cancelled = false;
        const cleanup: UnlistenFn[] = [];

        async function register() {
            const fns = await Promise.all([
                listen<{ text: string }>('transcript', (e) => {
                    console.log("[backend] transcript:", e);
                    addMessage('user', e.payload.text)
                }
                ),
                    listen<{ full_response: string; milestone: boolean }>('response_done', (e) =>{
                    console.log("[backend] response_done:", e);
                    finalizeStream(e.payload.full_response)}
                ),
                listen<{ greeting: string }>('session_ready', (e) =>{
                    console.log("[backend] session_ready:", e);
                    setSessionStatus('ready')}
                ),
                listen<{ message: string }>('error', (e) => {
                    console.log("[backend] error:", e);
                    console.error('[backend]', e.payload.message);
                    addMessage('system', `Error: ${e.payload.message}`);
                    setSessionStatus('idle');
                }),
            ]);

            if (cancelled) {
                fns.forEach((fn) => fn());
                return;
            }

            cleanup.push(...fns);
        }

        register();

        return () => {
            cancelled = true;
            cleanup.forEach((fn) => fn());
        };
    }, [addMessage, appendToken, finalizeStream, setSessionStatus]);
}
