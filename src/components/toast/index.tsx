import { useEffect } from 'react';
import { X } from 'lucide-react';
import './toast.css';
import { useAppStore } from '../../lib/store';
import type { Toast } from '../../lib/store';
import { Inline, Stack } from '../../design-system/primitives';
import Button from '../../design-system/components/button';

function ToastItem({ id, variant, message, duration }: Toast) {
    const removeToast = useAppStore((s) => s.removeToast);

    useEffect(function startDismissTimer() {
        const timer = setTimeout(function dismissToast() {
            removeToast(id);
        }, duration);

        return function cancelDismissTimer() {
            clearTimeout(timer);
        };
    }, [id, duration, removeToast]);

    return (
        <Inline
            variant={variant}
            variantAppearance="tonal"
            paint="surface"
            className="toast"
            justify="between"
            align="center"
            gap="sm"
            role="status"
        >
            <span className="toast__message">{message}</span>
            <Button
                className="toast__dismiss"
                onClick={() => removeToast(id)}
                aria-label="Dismiss"
                icon={X}
            />
        </Inline>
    );
}

export default function ToastContainer() {
    
    const toasts = useAppStore((s) => s.toasts);

    if (toasts.length === 0) return null;

    return (
        <Stack
            className="toast-container"
            aria-live="polite"
            aria-atomic="false"
            gap="sm"
        >
            {toasts.map((t) => (
                <ToastItem key={t.id} {...t} />
            ))}
        </Stack>
    );
}
