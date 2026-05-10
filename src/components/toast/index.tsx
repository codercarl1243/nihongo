import { useEffect } from 'react';
import './toast.css';
import { useAppStore } from '../../lib/store';
import type { Toast } from '../../lib/store';
import { Block, Inline } from '../../design-system/primitives';

function ToastItem({ id, variant, message, duration }: Toast) {
    const removeToast = useAppStore((s) => s.removeToast);

    useEffect(() => {
        const timer = setTimeout(() => removeToast(id), duration);
        return () => clearTimeout(timer);
    }, [id, duration, removeToast]);

    return (
        <Block
            variant={variant}
            variantAppearance="tonal"
            paint="surface"
            className="toast"
            role="status"
        >
            {/* justify-between requires plain div: <Inline> has no space-between prop */}
            <div className="toast__inner">
                <Inline gap="sm" align="center">
                    <span className="toast__message">{message}</span>
                </Inline>
                <button
                    className="toast__dismiss"
                    onClick={() => removeToast(id)}
                    aria-label="Dismiss"
                >
                    ✕
                </button>
            </div>
        </Block>
    );
}

export default function ToastContainer() {
    const toasts = useAppStore((s) => s.toasts);
    if (toasts.length === 0) return null;

    return (
        <div className="toast-container" aria-live="polite" aria-atomic="false">
            {toasts.map((t) => (
                <ToastItem key={t.id} {...t} />
            ))}
        </div>
    );
}
