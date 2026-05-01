import { logWarning } from "../../../lib/logging/log";
import { isNonEmptyString, isNonEmptyArray, isNullish } from "../../../lib/guards";
import { PAINT_CHANNELS, PAINT_PRESETS, type PaintChannel, type PaintPreset } from "../types/paint";
import { isOneOfTokens } from "./tokens";

function isPaintPreset(value: unknown): value is PaintPreset {
  return isOneOfTokens(PAINT_PRESETS, value)
}

function isPaintChannel(value: unknown): value is PaintChannel {
  return isOneOfTokens(PAINT_CHANNELS, value)
}

function extractPaintStrings(input: unknown): string[] {
  if (isNonEmptyString(input)) {
    return [input];
  }

  if (isNonEmptyArray(input)) {
    return input.filter(isNonEmptyString);
  }

  return [];
}

export default function validatePaint(paint: unknown) {
  if (isNullish(paint)) return;

  const values = extractPaintStrings(paint);
  if (values.length === 0) return;

  const hasPreset = values.some(isPaintPreset);
  const hasChannel = values.some(isPaintChannel);

  const unknownValues = values.filter(
    v => !isPaintPreset(v) && !isPaintChannel(v)
  );

  if (unknownValues.length > 0) {
    logWarning(
      `[Block] Unknown paint value(s): "${unknownValues.join(' ')}"`
    );
  }

  if (hasPreset && hasChannel) {
    logWarning(
      `[Block] Invalid paint usage: presets (${PAINT_PRESETS.join(
        ', '
      )}) must not be combined with channels (${PAINT_CHANNELS.join(', ')}).\n` +
      `Received: ${values.join(' ')}`
    );
  }
}
