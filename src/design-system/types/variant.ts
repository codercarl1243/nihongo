/**
 * Semantic color variants used across the design system.
 *
 * These variants map directly to design-token system
 * (e.g. `--color-primary-*`, `--text-on-primary`, etc.)
 *
 * Components like Button, Link, InlineCode, PostNote, Card, etc.
 * consume this type to ensure consistency across the entire UI.
 *
 * Example:
 * ```ts
 * <Button      variant="accent" />
 * <InlineCode  highlightVariant="warning" />
 * <PostNote    variant="info" />
 * ```
 */
export type Variant =
    | "accent"
    | "danger"
    | "dark"
    | "info"
    | "inverse"
    | "light"
    | "muted"
    | "neutral"
    | "primary"
    | "secondary"
    | "success"
    | "transparent"
    | "warning";

/**
 * Visual appearance variants that control *how* a component is rendered,
 * independent of its semantic color variant.
 *
 * These map to different visual treatments for interactive or decorative
 * components such as Button, Tag, Chip, Badge, etc.
 *
 * The meaning of each:
 *
 * - **filled** — Solid background with foreground text (most prominent)
 * - **outlined** — Transparent background with a 1px border
 * - **ghost** — Minimal chrome, subtle hover/focus treatments
 * - **tonal** — adds background to the component.
 *
 * Example:
 * ```ts
 * <Button  variant="primary"   variantAppearance="filled" />
 * <Button  variant="accent"    variantAppearance="outlined" />
 * <Row     variant="primary"   variantAppearance="tonal" />
 * ```
 */
export type VariantAppearance =
    | "filled"
    | "outlined"
    | "ghost"
    | "tonal";