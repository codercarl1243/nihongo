import { useAppStore } from "../../lib/store";

export default function TokenCount() {
    const promptTokens = useAppStore((s) => s.promptTokens);

    return (
        <p className="token-counter">
            {promptTokens} / 4096 tokens
        </p>
    );
}