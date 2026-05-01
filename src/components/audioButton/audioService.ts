import { useBrowserRecorder } from "./useBrowserRecorder";

export function useAudioRecorder() {
  // Later swap this line out for a Rust-based recorder that uses Tauri's native capabilities for better performance and reliability. i.e. CPAPS-based recording for low latency and high quality.
  return useBrowserRecorder();

  // TODO: maybe use pipecate once everything is working. this requires audio streaming though
  // return useRustRecorder();
}