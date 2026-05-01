import { type ChangeEvent } from "react";
import Button from "../../design-system/components/button";
import useChat from "./useChat";
import { Block, Stack } from "../../design-system/primitives";
import AudioButton from "../audioButton";
export default function ChatWindow() {
  const { state, handleSendMessage, handleSetMessage, scrollRef } = useChat();

  return (
    <Stack gap="lg" className="chat-window pt-md">
      <Stack
        gap="md"
        className="messages"
        role="log"
        aria-live="polite"
        ref={scrollRef}
        aria-label="Chat history"
      >
        {state.messageArray.map((message, index) => (
          <div key={index} className={`message ${message.sender}`}>
            {message.text}
          </div>
        ))}
      </Stack>
      <form className="input-form flow-md" onSubmit={handleSendMessage}>
        <Block
          as="input"
          variant="light"
          variantAppearance="filled"
          paint="all"
          type="text"
          placeholder="Type your message..."
          aria-label="Message input"
          value={state.message ?? ""}
          onChange={(e: ChangeEvent<HTMLInputElement>) => handleSetMessage(e.currentTarget.value)}
        />
        <Button type="submit">Send Message</Button>
      </form>
      <AudioButton />
    </Stack>
  );
}