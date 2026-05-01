type DataAttributeObjectType = Record<string, string | number | boolean | undefined | null>
    | undefined
    | null

type OutputType = Record<string, string>

const PRESERVE_TESTID_KEYS = new Set([
    "testid",
    "data-testid"
]);

/**
 * Converts one or more objects into React-safe `data-*` attribute pairs.
 *
 * This utility:
 * - Converts camelCase → kebab-case (`isActive` → `data-is-active`)
 * - Normalizes known testing keys (`testId`, `data-testid` → `data-testid`)
 * - Ignores `null` and `undefined` values
 * - Converts all values to strings (React requirement)
 *
 * pass any number of input objects, and the result merges all valid `data-*` attributes.
 *
 * ---
 * @example
 * // Basic usage
 * applyDataAttributes({
 *   theme: "dark",
 *   dataVariant: "primary",
 *   isActive: true
 * })
 * // → {
 * //   "data-theme": "dark",
 * //   "data-variant": "primary",
 * //   "data-is-active": "true"
 * // }
 *
 * @example
 * // Handles test IDs (React Testing Library + Jest compatibility)
 * applyDataAttributes({
 *   testId: "button-1"
 * })
 * // → { "data-testid": "button-1" }
 *
 * @example
 * // Filters out null/undefined safely
 * applyDataAttributes({
 *   size: undefined,
 *   theme: null,
 *   variant: "accent"
 * })
 * // → { "data-variant": "accent" }
 *
 * @example
 * // Merges multiple objects
 * applyDataAttributes(
 *   { variant: "primary" },
 *   null,
 *   { expanded: false, count: 3 }
 * )
 * // → {
 * //   "data-variant": "primary",
 * //   "data-expanded": "false",
 * //   "data-count": "3"
 * // }
 */

export function applyDataAttributes(
    ...sources: Array<DataAttributeObjectType>
): OutputType {
    const out: OutputType = {};

    for (const data of sources) {
        if (data == null) continue;

        for (const rawKey in data) {
            const value = data[rawKey];
            if (value == null) continue;

            const lowerKey = rawKey.toLowerCase();

            // Special case: normalize to data-testid
            if (PRESERVE_TESTID_KEYS.has(lowerKey)) {
                out["data-testid"] = String(value);
                continue;
            }

            // Strip "data-" or "data_" prefix
            const stripped = rawKey.replace(/^data[-_]*/i, "");

            // Convert camelCase → kebab-case, then lowercase
            const kebabKey = stripped
                .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
                .replace(/([A-Z])([A-Z][a-z])/g, "$1-$2")
                .toLowerCase();

            out[`data-${kebabKey}`] = String(value);
        }
    }

    return out;
}