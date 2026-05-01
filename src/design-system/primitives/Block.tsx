import type { ElementType } from "react";
import type { BlockProps } from "./types";
import { applyDataAttributes } from "../lib/applyDataAttributes";
import clsx from "clsx";
import validatePaint from "../lib/validatePaint";
import toPaintAttribute from "../lib/toPaintAttribute";

/**
 * Block — a lightweight, polymorphic primitive and styling boundary.
 *
 * Block renders the chosen element and exposes the design-system styling
 * contract (variant, appearance, paint) without applying layout, spacing,
 * or visual affordances by default.
 *
 * Styling is entirely opt-in:
 * - `variant` provides semantic color tokens
 * - `variantAppearance` defines how those tokens are mapped
 * - `paint` explicitly opts the element into painting
 *
 * This makes Block suitable as a semantic wrapper, structural grouping
 * element, or low-level foundation for higher-level components.
 *
 * @template T The HTML element or React component to render.
 *
 * @param {Object} props
 * @param {T} [props.as="div"] – The element type to render.
 * @param {Variant} [props.variant] – Semantic color variant to expose.
 * @param {VariantAppearance} [props.variantAppearance] – Defines how variant tokens are applied.
 * @param {PaintPreset | PaintChannel | PaintChannel[]} [props.paint] –
 *   Opt-in paint behavior. Presets apply common combinations; channels may be combined.
 * @param {string} [props.className] – Additional class names to apply.
 * @param {React.ReactNode} [props.children] – The component children.
 *
 * @example
 * // Semantic wrapper with no visual styling
 * <Block as="section">
 *   <Heading>Section</Heading>
 *   <p>Content…</p>
 * </Block>
 *
 * @example
 * // Tonal surface container
 * <Block
 *   className="surface"
 *   variant="info"
 *   variantAppearance="tonal"
 *   paint="surface"
 * >
 *   Informational content
 * </Block>
 *
 * @example
 * // Fully painted interactive element
 * <Block
 *   variant="primary"
 *   variantAppearance="filled"
 *   paint="all"
 * >
 *   Action
 * </Block>
 *
 * @example
 * // Variant tokens without paint (e.g. list markers)
 * <Block as="ul" variant="accent" className="list">
 *   <li>Item</li>
 *   <li>Item</li>
 * </Block>
 */

export default function Block<T extends ElementType = "div">({
    as,
    variant,
    variantAppearance,
    paint,
    className,
    children,
    container,
    ...props
}: BlockProps<T>) {

    const Component = as || "div";

    validatePaint(paint);
    const paintAttr = toPaintAttribute(paint);

    return <Component
        className={clsx("block", { "container": container }, className)}
        {...applyDataAttributes({ variant, appearance: variantAppearance, paint: paintAttr })}
        {...props}
    >
        {children}
    </Component>
}