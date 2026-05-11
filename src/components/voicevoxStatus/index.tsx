import { Stack, Inline } from '../../design-system/primitives';
import Spinner from '../../design-system/components/spinner';
import Progress from '../progress';
import { useAppStore } from '../../lib/store';
import type { VoiceVoxState } from '../../lib/store';
import './voicevoxStatus.css';

const LABEL: Record<VoiceVoxState, string> = {
    pending:     'Setting up text-to-speech…',
    checking:    'Checking text-to-speech…',
    downloading: 'Downloading TTS engine (first run only)…',
    extracting:  'Extracting TTS engine…',
    starting:    'Starting TTS engine…',
    ready:       '',
    error:       'TTS engine failed to start',
};

export default function VoicevoxStatus() {
    const state    = useAppStore((s) => s.voicevoxState);
    const progress = useAppStore((s) => s.voicevoxProgress);
    const message  = useAppStore((s) => s.voicevoxMessage);

    if (state === 'ready') return null;

    const label = message || LABEL[state] || 'Setting up text-to-speech…';
    const isError = state === 'error';

    return (
        <Stack gap="xs" className="voicevox-status">
            <Inline gap="sm" align="center" className="voicevox-status__row">
                {!isError && <Spinner />}
                <span className={`voicevox-status__label ${isError ? 'voicevox-status__label--error' : ''}`}>
                    {label}
                </span>
                {state === 'downloading' && progress > 0 && (
                    <span className="voicevox-status__pct">{progress}%</span>
                )}
            </Inline>

            {state === 'downloading' && (
                <Progress
                    value={progress}
                    size="sm"
                    aria-label="VoiceVox Engine download progress"
                />
            )}
        </Stack>
    );
}
