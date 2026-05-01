import type { KeyPressCallbackMap, KeyPressEventType } from "./keyboardHandlers.type"

/**
 * Invokes a registered callback for a keyboard event if a matching key or key combination
 * is found in the provided map.
 *
 * The function canonicalizes the event key:
 * - Single-character keys are converted to lowercase (e.g. "A" → "a")
 * - Named keys (e.g. "Enter", "Escape") are kept verbatim
 * - Modifier combinations are normalized (e.g. "Control+Enter", "Meta+k")
 *
 * It then looks up the callback in `keyMap` using the normalized key combination.
 * If a matching callback is found, the event's default action is prevented and the callback
 * is invoked with the original event.
 *
 * @param event - The keyboard event to handle.
 * @param keyMap - A mapping of key names or combinations to callback functions.
 *
 * @example
 * handleKeyPress(event, {
 *   Enter: () => console.log('Enter pressed'),
 *   'Control+Enter': () => console.log('Ctrl+Enter combo'),
 *   'Meta+k': () => openCommandPalette(),
 * });
 */
export function handleKeyPress(
  event: KeyPressEventType,
  keyMap: KeyPressCallbackMap
) {
  if (!event?.key || !keyMap || Object.keys(keyMap).length === 0) return;

  // Ignore events during IME composition (e.g., typing Japanese/Chinese/Korean)
  if (event.key === 'Process' || (event as KeyboardEvent).isComposing) return;

  const pressed = normalizeKeyPress(event.key, event);
  const match = Object.keys(keyMap).find(rawKey =>
    normalizeKeyString(rawKey) === pressed
  );

  if (match) {
    event.preventDefault();
    keyMap[match]?.(event);
  }
}

/**
 * Returns the canonical alias for a given key string.
 * Handles common alternate spellings and short forms.
 */
export function getKeyAlias(key: string): string {
  if (!key) return '';

  const lowerKey = key.toLowerCase();

  switch (lowerKey) {
    // Standard alternate spellings
    case ' ':
    case 'spacebar':
      return 'space';
    case 'esc':
      return 'escape';
    case 'del':
      return 'delete';
    case 'return':
      return 'enter';
    case 'ctrl':
      return 'control';

    // Arrow key short forms
    case 'left':
      return 'arrowleft';
    case 'right':
      return 'arrowright';
    case 'up':
      return 'arrowup';
    case 'down':
      return 'arrowdown';

    default:
      return lowerKey;
  }
}

/**
 * Normalizes a single string (like "Ctrl + Enter") into 
 * the canonical format (like "control+enter").
 */
function normalizeKeyString(rawKey: string): string {
  const tokens = rawKey.toLowerCase().split('+').map(t => t.trim());
  const modifiers = new Set<string>();
  let baseKey = '';

  for (const token of tokens) {
    const canonical = getKeyAlias(token);
    if (MODIFIER_ORDER.includes(canonical as any)) {
      modifiers.add(canonical);
    } else {
      baseKey = canonical;
    }
  }

  return toCanonicalCombo(baseKey, modifiers);
}


const MODIFIER_ORDER = ['control', 'meta', 'shift', 'alt'] as const;

/**
 * The "Single Source of Truth" for key formatting.
 * Takes raw tokens and returns a sorted, lowercased, aliased string.
 */
function toCanonicalCombo(baseKey: string, modifiers: Set<string>): string {
  const canonicalBase = getKeyAlias(baseKey);

  // If the 'base' is actually a modifier (e.g. user just pressed "Ctrl"), 
  // we treat the modifier list as the whole combo.
  if (MODIFIER_ORDER.includes(canonicalBase as any)) {
    modifiers.add(canonicalBase);
    const sorted = MODIFIER_ORDER.filter(m => modifiers.has(m));
    return sorted.join('+');
  }

  const sortedMods = MODIFIER_ORDER.filter(m => modifiers.has(m));
  return sortedMods.length > 0
    ? [...sortedMods, canonicalBase].join('+')
    : canonicalBase;
}
/**
 * Normalizes a key press into a canonical string representation.
 * Combines active modifiers with the key name, excluding modifier keys themselves.
 * 
 * For shifted symbols (e.g., Shift+/ produces '?'), this function derives the
 * unshifted key from event.code to ensure consistency with keyMap entries.
 * 
 * @example
 * normalizeKeyPress('Enter', event) // 'enter' (no modifiers)
 * normalizeKeyPress('k', event) // 'control+k' (with Ctrl pressed)
 * normalizeKeyPress('Control', event) // 'control' (modifier key alone)
 * normalizeKeyPress('?', event) // 'shift+/' (Shift+/ pressed, uses code)
 */
export function normalizeKeyPress(key: string, event: KeyPressEventType): string {
  if (!key) return '';

  const modifiers = new Set<string>();
  if (event.ctrlKey) modifiers.add('control');
  if (event.metaKey) modifiers.add('meta');
  if (event.shiftKey) modifiers.add('shift');
  if (event.altKey) modifiers.add('alt');

  let baseKey = key;
  // Use your existing logic for shifted symbols/numpad
  if (event.code?.startsWith('Numpad')) {
    baseKey = getKeyFromCode(event.code);
  } else if (event.shiftKey && event.code && isShiftedSymbol(key, event.code)) {
    baseKey = getKeyFromCode(event.code);
  }

  return toCanonicalCombo(baseKey, modifiers);
}


/**
 * Checks if a key represents a shifted symbol by comparing key length and code.
 * Shifted symbols are single characters that differ from their code representation.
 * 
 * @internal Exported for testing purposes
 */
export function isShiftedSymbol(key: string, code: string): boolean {
  // Single character that's likely a shifted symbol
  if (key.length !== 1) return false;

  // Check if code starts with 'Digit' or common punctuation codes
  if (code.startsWith('Digit')) return true;

  // Common punctuation keys that produce different characters when shifted
  const punctuationCodes = [
    'Slash', 'Backslash', 'BracketLeft', 'BracketRight',
    'Semicolon', 'Quote', 'Comma', 'Period', 'Minus', 'Equal',
    'Backquote'
  ];

  return punctuationCodes.includes(code);
}

/**
 * Converts a KeyboardEvent.code to its unshifted key character.
 * E.g., 'Slash' -> '/', 'Digit1' -> '1', 'KeyA' -> 'a'
 * 
 * Numpad keys are normalized to their main keyboard equivalents
 * (e.g., 'NumpadEnter' -> 'enter', 'Numpad1' -> '1').
 * 
 * @internal Exported for testing purposes
 */
export function getKeyFromCode(code: string): string {
  // Handle numpad keys - normalize to main keyboard equivalents
  if (code.startsWith('Numpad')) {
    const numpadToKey: Record<string, string> = {
      'NumpadEnter': 'enter',
      'NumpadAdd': '+',
      'NumpadSubtract': '-',
      'NumpadMultiply': '*',
      'NumpadDivide': '/',
      'NumpadDecimal': '.',
      'Numpad0': '0',
      'Numpad1': '1',
      'Numpad2': '2',
      'Numpad3': '3',
      'Numpad4': '4',
      'Numpad5': '5',
      'Numpad6': '6',
      'Numpad7': '7',
      'Numpad8': '8',
      'Numpad9': '9',
    };
    return numpadToKey[code] || code.toLowerCase();
  }

  // Handle digit keys (Digit0-Digit9)
  if (code.startsWith('Digit')) {
    return code.replace('Digit', '');
  }

  // Handle letter keys (KeyA-KeyZ)
  if (code.startsWith('Key')) {
    return code.replace('Key', '').toLowerCase();
  }

  // Map common punctuation codes to their unshifted characters
  const codeToKey: Record<string, string> = {
    'Slash': '/',
    'Backslash': '\\',
    'BracketLeft': '[',
    'BracketRight': ']',
    'Semicolon': ';',
    'Quote': "'",
    'Comma': ',',
    'Period': '.',
    'Minus': '-',
    'Equal': '=',
    'Backquote': '`',
  };

  return codeToKey[code]?.toLowerCase() || code.toLowerCase();
}