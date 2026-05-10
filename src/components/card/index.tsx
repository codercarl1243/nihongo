import './card.css';
import clsx from 'clsx';
import { Stack } from '../../design-system/primitives';
import { StackProps } from '../../design-system/primitives/types';

type CardProps = StackProps & {
    interactive?: boolean;
};

export default function Card({
    variant = 'neutral',
    variantAppearance = 'outlined',
    gap = 'lg',
    interactive = false,
    ...props
}: CardProps) {

    return (
        <Stack
            variant={variant}
            variantAppearance={variantAppearance}
            paint="surface"
            gap={gap}
            className={clsx('card', { 'card--interactive': interactive })}
            {...props}
        />
    );
}