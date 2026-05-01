import { useEffect, useReducer, useRef } from "react";
import { sanitizeString } from "../../../lib/string/sanitizeString";
import type { Message, ChatState, ChatAction } from "./type";

const testMessages: Message[] = [
    { sender: "user", text: "Hello, how are you?" },
    { sender: "bot", text: "I'm good, thank you! How can I assist you today?" },
    { sender: "user", text: "Can you tell me a joke?" },
    { sender: "bot", text: "Sure! Why don't scientists trust atoms? Because they make up everything!" },
];

const initialState: ChatState = {
    message: "",
    messageArray: [
        ...testMessages
    ],
};

function reducer(state: ChatState, action: ChatAction): ChatState {
    switch (action.type) {
        case "SET_MESSAGE":
            return { ...state, message: action.payload };
        case "SEND_MESSAGE":
            return {
                ...state,
                messageArray: [...state.messageArray, { sender: "user", text: action.payload }],
                message: "",
            };
        default:
            return state;
    }
}

export default function useChat() {
    const [state, dispatch] = useReducer(reducer, initialState);
    const scrollRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [state.messageArray]);
    const handleSetMessage = (message: string) => {
        dispatch({ type: "SET_MESSAGE", payload: message });
    };

    const handleSendMessage = (e: React.FormEvent) => {
        e.preventDefault();
        const sanitizedMessage = sanitizeString(state.message, {
            removeEmoji: true,
            normalizeWhitespace: true,
        });

        if (!sanitizedMessage || !sanitizedMessage.length) return;

        dispatch({ type: "SEND_MESSAGE", payload: sanitizedMessage });
    };

    return {
        state,
        scrollRef,
        handleSendMessage,
        handleSetMessage
    }
}