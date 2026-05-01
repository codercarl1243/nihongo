import { Paint } from "../types/paint";

export default function toPaintAttribute(paint: Paint | undefined): string | undefined {
    if (!paint) return undefined;

    if (Array.isArray(paint)) {
        return paint.length > 0 ? paint.join(' ') : undefined;
    }

    return paint;
}