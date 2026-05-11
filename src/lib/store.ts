import { create } from 'zustand';
import type { Variant } from '../design-system/types/variant';

export type SessionStatus = 'warming_up' | 'idle' | 'starting' | 'ready';

export type VoiceVoxState =
    | 'pending'     // before first event received
    | 'checking'
    | 'downloading'
    | 'extracting'
    | 'starting'
    | 'ready'
    | 'error';

export type Toast = {
    id: string;
    variant: Variant;
    message: string;
    duration: number;
};

export type Message = {
    id: string;
    sender: 'user' | 'tutor' | 'system';
    text: string;
};

type AppState = {
    sessionStatus: SessionStatus;
    voicevoxState: VoiceVoxState;
    voicevoxProgress: number;      // 0–100, meaningful during 'downloading'
    voicevoxMessage: string | null;
    messages: Message[];
    streamingText: string;
    promptTokens: number;
    micActive: boolean;
    isThinking: boolean;
    pipelineStage: string; // [PIPELINE_DEBUG]
    toasts: Toast[];

    setSessionStatus: (status: SessionStatus) => void;
    setVoicevoxStatus: (payload: { state: string; progress?: number | null; message?: string | null }) => void;
    addMessage: (sender: 'user' | 'tutor' | 'system', text: string) => void;
    appendToken: (token: string) => void;
    finalizeStream: (fullText: string) => void;
    setPromptTokens: (n: number) => void;
    setMicActive: (active: boolean) => void;
    setIsThinking: (thinking: boolean) => void;
    setPipelineStage: (stage: string) => void; // [PIPELINE_DEBUG]
    addToast: (opts: { variant: Variant; message: string; duration?: number }) => void;
    removeToast: (id: string) => void;
};

export const useAppStore = create<AppState>((set) => ({
    sessionStatus: 'warming_up',
    voicevoxState: 'pending',
    voicevoxProgress: 0,
    voicevoxMessage: null,
    messages: [],
    streamingText: '',
    promptTokens: 0,
    micActive: false,
    isThinking: false,
    pipelineStage: '', // [PIPELINE_DEBUG]
    toasts: [],

    setSessionStatus: (sessionStatus) => set({ sessionStatus }),

    setVoicevoxStatus: ({ state, progress, message }) =>
        set({
            voicevoxState:    state as VoiceVoxState,
            voicevoxProgress: progress != null ? Math.round(progress * 100) : 0,
            voicevoxMessage:  message ?? null,
        }),

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
    addToast: ({ variant, message, duration = 4000 }) =>
        set((s) => ({
            toasts: [
                ...s.toasts,
                { 
                    id: crypto.randomUUID(), 
                    variant, 
                    message, 
                    duration: Number.isFinite(duration) && duration > 0 ? duration : 4000
                }],
        })),
    removeToast: (id) =>
        set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));
