// [PIPELINE_DEBUG] — delete this file when pipeline indicators are no longer needed

import { Fragment } from 'react';

const STAGES = [
    'VAD 1: Listening',
    'VAD 1: Speech Detected',
    'VAD 1: Silence Detected',
    'ASR: Transcribing',
    'LLM: Generating',
    'TTS: Synthesizing',
    'VAD 2: Barge-In Detected',
    'Audio: Playing',
    'DB: Writing',
] as const;

interface PipelineFlowProps {
    stage: string;
}

export default function PipelineFlow({ stage }: PipelineFlowProps) {
    return (
        <div className="pipeline-flow">
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
        </div>
    );
}
