import clsx from "clsx";
import { Block } from "../../design-system/primitives";
import type { BlockWrapperProps } from "../../design-system/primitives/types";
import type { Message } from "../../lib/store";

type ChatMessageProps = BlockWrapperProps<'p', Omit<Message, 'id'>>; 

export default function ChatMessage({ text, sender, className }: ChatMessageProps) {
    return (
        <Block as="p" className={clsx(`message ${sender}`, className)}>
            {text}
        </Block>
    );
}