import Button from '../../design-system/components/button';
import { Inline } from '../../design-system/primitives';
import { bargeIn } from '../../lib/api';
import type { SessionStatus } from '../../lib/store';

type Props = {
    status: SessionStatus;
    voicevoxReady: boolean;
    onStart: () => Promise<void>;
    onStop: () => Promise<void>;
};

export default function SessionButton({ status, voicevoxReady, onStart, onStop }: Props) {
    if (status === 'warming_up') {
        return <Button isLoading>Warming Up…</Button>;
    }

    if (status === 'idle') {
        // Block session start until TTS engine is ready — first-run download may still be in progress.
        if (!voicevoxReady) {
            return <Button isLoading>Setting Up TTS…</Button>;
        }
        return <Button onClick={onStart}>Start Session</Button>;
    }

    if (status === 'starting') {
        return <Button isLoading>Starting…</Button>;
    }

    return (
        <Inline gap="sm">
            <Button onClick={bargeIn}>Stop Speaking</Button>
            <Button onClick={onStop}>End Session</Button>
        </Inline>
    );
}
