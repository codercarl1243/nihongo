type AudioRecorder = {
  start: () => Promise<void>;
  stop: () => Promise<Blob>;
  recording: boolean;
};