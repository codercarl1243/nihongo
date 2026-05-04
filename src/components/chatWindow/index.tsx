import { Block, Stack } from '../../design-system/primitives';
import { useAppStore } from '../../lib/store';
import useChat from './useChat';
import SessionButton from '../audioButton';
import PipelineFlow from '../pipelineFlow'; // [PIPELINE_DEBUG]

export default function ChatWindow() {
    const { sessionStatus, messages, streamingText, scrollRef, start, stop } = useChat();
    const promptTokens  = useAppStore((s) => s.promptTokens);
    const micActive     = useAppStore((s) => s.micActive);
    const isThinking    = useAppStore((s) => s.isThinking);
    const pipelineStage = useAppStore((s) => s.pipelineStage); // [PIPELINE_DEBUG]

    return (
        <Stack gap="lg" className="chat-window pt-md">
            <Stack
                gap="md"
                className="messages"
                role="log"
                aria-live="polite"
                aria-label="Chat history"
            >
                {messages.map((msg) => (
                    <Block key={msg.id} className={`message ${msg.sender}`}>
                        {msg.text}
                    </Block>
                ))}

                {streamingText && (
                    <Block className="message tutor streaming">
                        {streamingText}
                    </Block>
                )}

                <div ref={scrollRef} />
            </Stack>

            {sessionStatus === 'ready' && (
                <div className="session-indicators">
                    <div className={`mic-indicator ${micActive ? 'listening' : 'suppressed'}`}>
                        {micActive ? 'Listening' : isThinking ? 'Thinking…' : 'Speaking'}
                    </div>
                </div>
            )}

            {sessionStatus === 'ready' && (
                <PipelineFlow stage={pipelineStage} /> // [PIPELINE_DEBUG]
            )}

            <SessionButton status={sessionStatus} onStart={start} onStop={stop} />

            {sessionStatus === 'ready' && (
                <div className="token-counter">{promptTokens} / 4096 tokens</div>
            )}
        </Stack>
    );
}
