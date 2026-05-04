import { create } from 'zustand';

export type SessionStatus = 'warming_up' | 'idle' | 'starting' | 'ready';

export type Message = {
    id: string;
    sender: 'user' | 'tutor' | 'system';
    text: string;
};

type AppState = {
    sessionStatus: SessionStatus;
    messages: Message[];
    streamingText: string;
    promptTokens: number;
    micActive: boolean;
    isThinking: boolean;
    pipelineStage: string; // [PIPELINE_DEBUG]

    setSessionStatus: (status: SessionStatus) => void;
    addMessage: (sender: 'user' | 'tutor' | 'system', text: string) => void;
    appendToken: (token: string) => void;
    finalizeStream: (fullText: string) => void;
    setPromptTokens: (n: number) => void;
    setMicActive: (active: boolean) => void;
    setIsThinking: (thinking: boolean) => void;
    setPipelineStage: (stage: string) => void; // [PIPELINE_DEBUG]
};

export const useAppStore = create<AppState>((set) => ({
    sessionStatus: 'warming_up',
    messages: [],
    streamingText: '',
    promptTokens: 0,
    micActive: false,
    isThinking: false,
    pipelineStage: '', // [PIPELINE_DEBUG]

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

    setPromptTokens: (promptTokens) => set({ promptTokens }),
    setMicActive: (micActive) => set({ micActive }),
    setIsThinking: (isThinking) => set({ isThinking }),
    setPipelineStage: (pipelineStage) => set({ pipelineStage }), // [PIPELINE_DEBUG]
}));
