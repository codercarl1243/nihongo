import './badge.css';
import clsx from 'clsx';
import { Block } from '../../design-system/primitives';
import { BlockWrapperProps } from '../../design-system/primitives/types';

type BaseBadgeProps = {
    size?: 'sm' | 'md';
};

type BadgeProps = BlockWrapperProps<'span', BaseBadgeProps>;

export default function Badge({
    variant = 'neutral',
    variantAppearance = 'tonal',
    size = 'md',
    ...props
}: BadgeProps) {
    return (
        <Block
            as="span"
            variant={variant}
            variantAppearance={variantAppearance}
            paint="surface"
            className={clsx('badge', `badge--${size}`)}
            {...props}
        />
    );
}
