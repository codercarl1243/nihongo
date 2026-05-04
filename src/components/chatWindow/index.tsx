import { Block, Stack } from '../../design-system/primitives';
import { useAppStore } from '../../lib/store';
import useChat from './useChat';
import SessionButton from '../audioButton';

export default function ChatWindow() {
    const { sessionStatus, messages, streamingText, scrollRef, start, stop } = useChat();
    const promptTokens = useAppStore((s) => s.promptTokens);

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

            <SessionButton status={sessionStatus} onStart={start} onStop={stop} />

            {sessionStatus === 'ready' && (
                <div className="token-counter">{promptTokens} / 4096 tokens</div>
            )}
        </Stack>
    );
}
