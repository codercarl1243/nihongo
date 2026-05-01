import clsx from "clsx";
import type { ElementType } from "react";
import type { BlockProps, StackProps } from "./types"; 
import Block from "./Block";

/**
 * Stack — a lightweight polymorphic layout wrapper.
 *
 * This component provides consistent vertical spacing between its children
 * using gap-row-* utility classes, while allowing polymorphic choice of a HTML element via the `as` prop.
 *
 * @template T The HTML element or React component to render.
 *
 * @param {Object} props
 * @param {T} [props.as="div"] – The element type to render. Defaults to `<div>`.
 * @param {0 | "sm" | "md" | "lg" | "xl" | "xxl"} [props.gap="lg"] – Vertical spacing between children.
 * @param {"start" | "center" | "end" | "stretch" | "baseline"} [props.align] – Vertical alignment of items.
 * @param {"start" | "center" | "end" | "stretch"} [props.justify] – Horizontal justification of items.
 * @param {string} [props.className] – Additional class names to apply.
 * @param {React.ReactNode} [props.children] – The component children.
 *
 * @example
 * // Basic usage
 * <Stack>
 *   <p>One</p>
 *   <p>Two</p>
 * </Stack>
 *
 * @example
 * // Custom element
 * <Stack as="section" gap="xl">
 *   <Heading>Section Title</Heading>
 *   <p>Some content…</p>
 * </Stack>
 *
 * @example
 * // With additional classes
 * <Stack className="surface-frame p-lg">
 *   <p>Styled container</p>
 * </Stack>
 */
export default function Stack<T extends ElementType = "div">({
    gap = "lg",
    align,
    justify,
    className,
    ...blockProps
}: StackProps<T>) {
 
    const classes = clsx(
        "stack",
        gap !== 0 ? `gap-row-${gap}` : "gap-row-0",
        align ? `stack-align-${align}` : "",
        justify ? `stack-justify-${justify}` : "",
        className
    );
 
    return (
        <Block
            className={classes}
            {...(blockProps as BlockProps<T>)}
        />
    );
}