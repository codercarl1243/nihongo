import emojiRegex from 'emoji-regex';
import { isNullish } from '../guards';

/**
 * Collection of string utility functions for sanitizing and transforming text input.
 * All functions handle null/undefined input gracefully by returning empty strings.
 */
type StringFunctionType = {
    /**
     * Escapes special regex characters in a string to make it safe for use in RegExp constructor.
     * Does NOT trim whitespace - that's left to the caller.
     * 
     * @param str - The string to escape
     * @returns The escaped string, or empty string if input is null/undefined
     * 
     * @example
     * ```ts
     * stringUtils.escape('test.*'); // Returns: 'test\\.\\*'
     * stringUtils.escape('price$100'); // Returns: 'price\\$100'
     * stringUtils.escape(null); // Returns: ''
     * ```
     */
    escape(str: string | null | undefined): string;

    /**
     * Normalizes whitespace by collapsing multiple spaces/tabs/newlines into single spaces
     * and trimming leading/trailing whitespace.
     * 
     * @param str - The string to normalize
     * @returns The normalized string with single spaces, or empty string if input is null/undefined
     * 
     * @example
     * ```ts
     * stringUtils.normalizeWhiteSpace('hello    world'); // Returns: 'hello world'
     * stringUtils.normalizeWhiteSpace('  test\n\nquery  '); // Returns: 'test query'
     * stringUtils.normalizeWhiteSpace('   '); // Returns: ''
     * ```
     */
    normalizeWhiteSpace(str: string | null | undefined): string;

    /**
     * Removes all emoji characters from a string using the emoji-regex library.
     * Handles various emoji categories including faces, objects, flags, and skin tones.
     * 
     * @param str - The string to remove emoji from
     * @returns The string without emoji, or empty string if input is null/undefined
     * 
     * @example
     * ```ts
     * stringUtils.removeEmoji('Hello 🎉 World 🚀'); // Returns: 'Hello  World '
     * stringUtils.removeEmoji('Test 😀😃😄'); // Returns: 'Test '
     * stringUtils.removeEmoji('🎉🚀'); // Returns: ''
     * ```
     */
    removeEmoji(str: string | null | undefined): string;

    /**
     * Removes all punctuation and special characters, keeping only letters, numbers, and spaces.
     * Uses Unicode-aware patterns (\p{L} and \p{N}) to preserve international characters.
     * 
     * @param str - The string to remove punctuation from
     * @returns The string without punctuation, or empty string if input is null/undefined
     * 
     * @example
     * ```ts
     * stringUtils.removePunctuation('Hello, World!'); // Returns: 'Hello World'
     * stringUtils.removePunctuation('user@email.com'); // Returns: 'useremailcom'
     * stringUtils.removePunctuation('Café'); // Returns: 'Café' (preserves accents)
     * stringUtils.removePunctuation('日本語'); // Returns: '日本語' (preserves international chars)
     * ```
     */
    removePunctuation(str: string | null | undefined): string;

    /**
     * Truncates a string to a maximum number of grapheme clusters (user-perceived characters).
     * Uses Intl.Segmenter when available to handle emoji and multi-byte characters correctly.
     * Falls back to string slicing in older environments.
     * 
     * @param str - The string to truncate
     * @param maxLength - Maximum number of grapheme clusters to keep
     * @returns The truncated string, or empty string if input is null/undefined or maxLength <= 0
     * 
     * @example
     * ```ts
     * stringUtils.truncate('hello world', 5); // Returns: 'hello'
     * stringUtils.truncate('Hello 🎉 World', 7); // Returns: 'Hello 🎉' (emoji counted as 1 character)
     * stringUtils.truncate('test', 10); // Returns: 'test' (no truncation needed)
     * stringUtils.truncate('test', 0); // Returns: ''
     * ```
     */
    truncate(str: string | null | undefined, maxLength: number): string;
    
    /**
     * Composes multiple string transformation functions into a single function.
     * 
     * @param functions - An array of functions to compose
     * @returns A function that applies all the provided functions in sequence
     * 
     * @example
     * ```ts
     * const cleanSearch = stringUtils.pipe(
     *   stringUtils.removeEmoji,
     *   stringUtils.removePunctuation,
     *   stringUtils.normalizeWhiteSpace
     * );
     * ```
     */
    pipe: (...functions: ((s: string) => string)[]) => (input: string | null | undefined) => string;
};

/**
 * String utility functions for text sanitization and transformation.
 * All functions safely handle null/undefined input and are designed to be composable.
 */
export const stringUtils: StringFunctionType = {
    escape: (str) => (!str ? '' : str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),

    normalizeWhiteSpace: (str) =>
        isNullish(str) ? '' : str.replace(/\s+/g, ' ').trim(),

    removeEmoji: (str) => isNullish(str) ? '' : str.replace(emojiRegex(), ''),

    removePunctuation: (str) =>
        isNullish(str) ? '' : str.replace(/[^\p{L}\p{N}\s]/gu, ''),

    truncate: (str: string | null | undefined, maxLength: number): string => {
        if (isNullish(str) || maxLength <= 0) return '';

        // Use Intl.Segmenter if available to handle emojis and grapheme clusters
        if (typeof Intl !== 'undefined' && 'Segmenter' in Intl) {
            const segmenter = typeof Intl !== 'undefined' && 'Segmenter' in Intl
                ? new Intl.Segmenter(undefined, { granularity: 'grapheme' })
                : null;
            if (!segmenter) return str.slice(0, maxLength); // Fallback if Segmenter isn't supported for some reason
            const graphemes = [...segmenter.segment(str)];
            return graphemes.slice(0, maxLength).map(seg => seg.segment).join('');
        }

        // Fallback for older environments
        return str.slice(0, maxLength);
    },

    pipe:
    (...fns: Array<(s: string) => string>) =>
    (input: string | null | undefined) =>
        fns.reduce((acc, fn) => fn(acc), input ?? ''),
};