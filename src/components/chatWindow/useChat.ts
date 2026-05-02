import { useEffect, useRef } from 'react';
import { useAppStore } from '../../lib/store';
import { startSession, stopSession } from '../../lib/api';
import useEvents from '../../lib/useEvents';

export default function useChat() {
    const sessionStatus    = useAppStore((s) => s.sessionStatus);
    const messages         = useAppStore((s) => s.messages);
    const streamingText    = useAppStore((s) => s.streamingText);
    const setSessionStatus = useAppStore((s) => s.setSessionStatus);
    const addMessage       = useAppStore((s) => s.addMessage);
    const scrollRef = useRef<HTMLDivElement>(null);

    useEvents();

    useEffect(() => {
        scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages, streamingText]);

    async function start() {
        setSessionStatus('starting');
        try {
            await startSession();
        } catch (e) {
            setSessionStatus('idle');
            addMessage('system', `Error: ${e}`);
        }
    }

    async function stop() {
        await stopSession();
        setSessionStatus('idle');
    }

    return { sessionStatus, messages, streamingText, scrollRef, start, stop };
}
