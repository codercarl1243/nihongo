// [PIPELINE_DEBUG] — delete this file when pipeline indicators are no longer needed

import { Fragment } from 'react';
import { Inline } from '../../design-system/primitives';

// Echo: Suppressing is emitted by the backend but intentionally omitted from this
// display — it's an internal AudioManager detail, not a user-visible pipeline step.
const STAGES = [
    'VAD 1: Listening',
    'VAD 1: Speech Detected',
    'VAD 1: Silence Detected',
    'ASR: Transcribing',
    'LLM: Generating',
    'TTS: Synthesizing',
    'VAD 2: Barge-In Detected',
    'Audio: Playing',
    'LLM: Classifying',
    'DB: Writing',
    'LLM: Compacting',
] as const;

interface PipelineFlowProps {
    stage: string;
}

export default function PipelineFlow({ stage }: PipelineFlowProps) {
    return (
        <Inline align="center" variant="neutral" variantAppearance='outlined' paint="foreground" className="pipeline-flow py-sm px-0">
            {STAGES.map((s, i) => (
                <Fragment key={s}>
                    <span className={`pipeline-step${s === stage ? ' active' : ''}`}>
                        {s}
                    </span>
                    {i < STAGES.length - 1 && (
                        <span className="pipeline-arrow">→</span>
                    )}
                </Fragment>
            ))}
        </Inline>
    );
}
