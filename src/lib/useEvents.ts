import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export default function useEvents() {
    const queryClient = useQueryClient();

    useEffect(function listenForEventsUseEffect() {
        const listenEvents: UnlistenFn[] = [];

        const registerEvents = async () => {
            // update Zustand immediately,
            // then invalidate React Query so it refetches in the background
            const unlisten = await listen('audio-transcribed', () => {
                queryClient.invalidateQueries({ queryKey: ['transcription'] });
            });
            listenEvents.push(unlisten);
        };

        registerEvents();

        function _listenForEventsUseEffect_Cleanup() {
            listenEvents.forEach((fn) => fn());
        }

        return _listenForEventsUseEffect_Cleanup;
    }, [queryClient])

}   