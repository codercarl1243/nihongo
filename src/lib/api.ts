import { invoke } from '@tauri-apps/api/core';

export async function startSession(): Promise<void> {
    return invoke('start_session');
}

export async function stopSession(): Promise<void> {
    return invoke('stop_session');
}

export async function bargeIn(): Promise<void> {
    return invoke('barge_in');
}

export async function getSidecarReady(): Promise<boolean> {
    return invoke('get_sidecar_ready');
}
