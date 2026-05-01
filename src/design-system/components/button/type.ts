import type { MouseEvent } from 'react'
import type { IconProps } from '../../components/icon/icon.type';
import type { BlockWrapperProps } from '../../primitives/types';
import type { AccessibleLabel } from '../../types/accessibility';

export type ButtonMouseEvent = MouseEvent<HTMLButtonElement>;

export type ButtonClickHandler = (event: ButtonMouseEvent) => void | Promise<void>;

type BaseButtonProps = {
  disabled?: boolean;
  isLoading?: boolean;
  icon?: IconProps['icon'];
  onClick?: ButtonClickHandler;
  showSpinner?: boolean;
};

export type ButtonProps = Omit<BlockWrapperProps<"button", BaseButtonProps>, 'as'>

export type IconButtonProps = AccessibleLabel & Omit<ButtonProps, 'children'>;

