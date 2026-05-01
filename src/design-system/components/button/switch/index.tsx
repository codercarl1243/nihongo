import clsx from "clsx";
import type { TSwitchProps } from "./type";
import Button from "../index";
import { ButtonProps } from "../type";

export default function Switch({
    checked,
    variant = "inverse",
    variantAppearance = "filled",
    children,
    className,
    ...props }: TSwitchProps) {

    const internalProps: ButtonProps = {
        ...props,
        variant,
        variantAppearance,
        role: "switch",
        "aria-checked": checked,
        className: clsx(className, 'switch'),
    };

    return <Button {...internalProps}>{children}</Button>;
}