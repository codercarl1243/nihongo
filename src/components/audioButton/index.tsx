import Button from "../../design-system/components/button";
import { transcribeAudio } from "../../lib/api";
import { resolvePath } from "../../lib/file";
import { useAudioRecorder } from "./audioService";

export default function Audio() {
    const { start, stop, recording, saveAudio } = useAudioRecorder();

    const handleVoice = async () => {
        const blob = await stop();

        const fileName = await saveAudio(blob);
        console.log("fileName: ", fileName);
const resolvedPath = await resolvePath(fileName);
console.log("Resolved path in component: ", resolvedPath);
        const text = await transcribeAudio(fileName);

        console.log(text);
    };

    return (
        <Button
            onPointerDown={start}
            onPointerUp={handleVoice}
        >
            {recording ? "Stop Recording" : "Start Recording"}
        </Button>
    );
}