import './card.css';
import clsx from 'clsx';
import type { ElementType } from 'react';
import { Block, Stack } from '../../design-system/primitives';
import type { Variant, VariantAppearance } from '../../design-system/types/variant';
import type { Spacing } from '../../design-system/types/spacing';

type CardProps = {
    variant?: Variant;
    appearance?: VariantAppearance;
    padding?: Spacing;
    interactive?: boolean;
    as?: ElementType;
    children: React.ReactNode;
};

export default function Card({
    variant = 'neutral',
    appearance = 'outlined',
    padding = 'lg',
    interactive = false,
    as,
    children,
}: CardProps) {
    return (
        <Block
            as={as}
            variant={variant}
            variantAppearance={appearance}
            paint="surface"
            className={clsx('card', { 'card--interactive': interactive })}
        >
            <Stack gap={padding}>
                {children}
            </Stack>
        </Block>
    );
}
