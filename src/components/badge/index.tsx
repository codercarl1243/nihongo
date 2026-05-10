import './badge.css';
import clsx from 'clsx';
import { Block } from '../../design-system/primitives';
import type { Variant, VariantAppearance } from '../../design-system/types/variant';

type BadgeProps = {
    variant?: Variant;
    appearance?: VariantAppearance;
    size?: 'sm' | 'md';
    children: React.ReactNode;
};

export default function Badge({
    variant = 'neutral',
    appearance = 'tonal',
    size = 'md',
    children,
}: BadgeProps) {
    return (
        <Block
            as="span"
            variant={variant}
            variantAppearance={appearance}
            paint="surface"
            className={clsx('badge', `badge--${size}`)}
        >
            {children}
        </Block>
    );
}
