/**
 * Paint presets represent high-level paint modes.
 *
 * They are NOT individual channels — they describe a predefined
 * combination of channels applied together.
 *
 * Examples:
 * - "all"     → apply foreground, background, and border
 * - "surface" → apply background and border (no text coloring)
 *
 * Presets are intentionally kept separate from channels because:
 * - they behave differently
 * - they are passed differently
 * - they must NOT be treated as individual paint targets
 */
export const PAINT_PRESETS = ['all', 'surface'] as const;

/**
 * Paint channels represent individual styling targets.
 *
 * Each channel controls a specific visual layer:
 * - "background" → the button background
 * - "foreground" → the text / icon color
 * - "border"     → the outline or stroke
 *
 * Channels are composable (can be combined in arrays),
 * unlike presets which are singular modes.
 */
export const PAINT_CHANNELS = ['background', 'foreground', 'border'] as const;

export type PaintPreset = typeof PAINT_PRESETS[number];
export type PaintChannel = typeof PAINT_CHANNELS[number];

export type Paint =
  | PaintPreset
  | PaintChannel
  | PaintChannel[];
