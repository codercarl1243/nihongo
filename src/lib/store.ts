import { create } from 'zustand';

type AppState = {
  transcription: string;
  setTranscription: (transcription: string) => void;
};

export const useAppStore = create<AppState>((set) => ({
  transcription: '',
  setTranscription: (transcription) => set({ transcription }),
}));    