import { ButtonProps } from "../type";

export type TSwitchProps = {
    checked: boolean;
} & Omit<ButtonProps, "role" | "aria-checked">;

