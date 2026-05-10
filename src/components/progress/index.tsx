import './progress.css';
import clsx from 'clsx';
import { Block } from '../../design-system/primitives';
import { BlockWrapperProps } from '../../design-system/primitives/types';

type BaseProgressProps = {
    value: number;
    size?: 'sm' | 'md';
    'aria-label': string;
};

type ProgressProps = BlockWrapperProps<'div', BaseProgressProps>;

export default function Progress({
    value,
    variant = 'primary',
    size = 'md',
    'aria-label': ariaLabel,
}: ProgressProps) {
    return (
        <Block
            variant={variant}
            className={clsx('progress-track', `progress-track--${size}`)}
        >
            <progress
                className="progress"
                value={Math.max(0, Math.min(100, value))}
                max={100}
                aria-label={ariaLabel}
            />
        </Block>
    );
}
