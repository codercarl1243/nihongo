import { create } from 'zustand';

export type SessionStatus = 'idle' | 'starting' | 'ready';

export type Message = {
    id: string;
    sender: 'user' | 'tutor' | 'system';
    text: string;
};

type AppState = {
    sessionStatus: SessionStatus;
    messages: Message[];
    streamingText: string;

    setSessionStatus: (status: SessionStatus) => void;
    addMessage: (sender: 'user' | 'tutor' | 'system', text: string) => void;
    appendToken: (token: string) => void;
    finalizeStream: (fullText: string) => void;
};

export const useAppStore = create<AppState>((set) => ({
    sessionStatus: 'idle',
    messages: [],
    streamingText: '',

    setSessionStatus: (sessionStatus) => set({ sessionStatus }),

    addMessage: (sender, text) =>
        set((s) => ({
            messages: [...s.messages, { id: crypto.randomUUID(), sender, text }],
        })),

    appendToken: (token) =>
        set((s) => ({ streamingText: s.streamingText + token })),

    finalizeStream: (fullText) =>
        set((s) => ({
            messages: [
                ...s.messages,
                { id: crypto.randomUUID(), sender: 'tutor', text: fullText },
            ],
            streamingText: '',
        })),
}));
