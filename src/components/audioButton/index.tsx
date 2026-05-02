import Button from '../../design-system/components/button';
import { Inline } from '../../design-system/primitives';
import { bargeIn } from '../../lib/api';
import type { SessionStatus } from '../../lib/store';

type Props = {
    status: SessionStatus;
    onStart: () => Promise<void>;
    onStop: () => Promise<void>;
};

export default function SessionButton({ status, onStart, onStop }: Props) {
    if (status === 'idle') {
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
