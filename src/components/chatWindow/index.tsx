import { Stack } from '../../design-system/primitives';
import { useAppStore } from '../../lib/store';
import useChat from './useChat';
import SessionButton from '../audioButton';
import PipelineFlow from '../pipelineFlow'; // [PIPELINE_DEBUG]
import ChatMessage from './chatMessage';
import TokenCount from './tokenCount';

export default function ChatWindow() {
    const { sessionStatus, messages, streamingText, scrollRef, start, stop } = useChat();
    const micActive     = useAppStore((s) => s.micActive);
    const isThinking    = useAppStore((s) => s.isThinking);
    const pipelineStage = useAppStore((s) => s.pipelineStage); // [PIPELINE_DEBUG]

    return (
        <Stack 
        gap="lg" 
        className="chat-window pt-md surface-frame my-lg"
        variant='inverse'
        variantAppearance='filled'
        paint="all"
        >
            <Stack
                gap="md"
                className="messages px-xs"
                role="log"
                aria-live="polite"
                aria-label="Chat history"
            >
                {messages.map((msg) => (
                    <ChatMessage key={msg.id} text={msg.text} sender={msg.sender} />
                ))}

                {streamingText && (
                    <ChatMessage className="streaming" text={streamingText} sender={'tutor'} />
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

            {sessionStatus === 'ready' && <TokenCount />}
        </Stack>
    );
}
