import clsx from "clsx";
import type { ElementType } from "react";
import type { BlockProps, InlineProps } from "./types";
import Block from "./Block";

/**
 * Inline — horizontal flow layout, optionally wrapping.
 *
 * Places children in a horizontal flow that participates in document layout.
 * Inline does not own its width — it takes up only as much space
 * as its children need. Ideal for icon-text pairs, tags, badges, and metadata clusters.
 *
 * @template T The HTML element or React component to render.
 *
 * @param {Object} props
 * @param {T} [props.as="div"] – The element type to render. Defaults to `<div>`.
 * @param {0 | "sm" | "md" | "lg" | "xl" | "xxl"} [props.gap="lg"] – Horizontal spacing between children.
 * @param {"start" | "center" | "end" | "stretch" | "baseline"} [props.align="baseline"] – Cross-axis alignment of items.
 *   Defaults to "baseline" — most inline groupings (icon-text pairs, tags) expect baseline alignment.
 * @param {"start" | "center" | "end" | "stretch" | "even" | "between" | "initial"} [props.justify="initial"] – Main-axis distribution of items.
 * @param {boolean} [props.wrap=true] – Whether children wrap to the next line. Defaults to true.
 * @param {string} [props.className] – Additional class names to apply.
 * @param {React.ReactNode} [props.children] – The component children.
 *
 * @example
 * // Basic usage — icon and text pair
 * <Inline>
 *   <Icon icon={RiCheckLine} />
 *   <span>Saved</span>
 * </Inline>
 *
 * @example
 * // Tag list with wrapping
 * <Inline gap="sm" wrap>
 *   <Tag>Design</Tag>
 *   <Tag>CSS</Tag>
 *   <Tag>React</Tag>
 * </Inline>
 *
 * @example
 * // Custom element — rendered as a nav
 * <Inline as="nav" gap="md" align="center">
 *   <Link href="/">Home</Link>
 *   <Link href="/blog">Blog</Link>
 *   <Link href="/about">About</Link>
 * </Inline>
 *
 * @example
 * // No wrap — single line only
 * <Inline wrap={false} gap="sm">
 *   <span>Step 1</span>
 *   <span>Step 2</span>
 *   <span>Step 3</span>
 * </Inline>
 */
export default function Inline<T extends ElementType = "div">({
    gap = "lg",
    align = "baseline",
    justify = "initial",
    wrap = true,
    className,
    ...blockProps
}: InlineProps<T>) {

    const classes = clsx(
        "inline-flow",
        wrap ? "inline-wrap" : "inline-nowrap",
        gap !== 0 ? `gap-${gap}` : "gap-0",
        `inline-align-${align}`,
        `inline-justify-${justify}`,
        className
    );

    return <Block
        className={classes}
        {...(blockProps as BlockProps<T>)}
    />;
}