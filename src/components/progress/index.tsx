import './progress.css';
import clsx from 'clsx';
import { Block } from '../../design-system/primitives';
import type { Variant } from '../../design-system/types/variant';

type ProgressProps = {
    value: number;
    variant?: Variant;
    size?: 'sm' | 'md';
    'aria-label': string;
};

export default function Progress({
    value,
    variant = 'primary',
    size = 'md',
    'aria-label': ariaLabel,
}: ProgressProps) {
    return (
        <Block variant={variant} className={clsx('progress-track', `progress-track--${size}`)}>
            <progress
                className="progress"
                value={Math.max(0, Math.min(100, value))}
                max={100}
                aria-label={ariaLabel}
            />
        </Block>
    );
}
