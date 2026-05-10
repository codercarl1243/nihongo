import { useAppStore } from '../../lib/store';
import type { Variant } from '../../design-system/types/variant';

export function useToast() {
    const addToast = useAppStore((s) => s.addToast);
    return {
        show: (opts: { variant: Variant; message: string; duration?: number }) =>
            addToast(opts),
    };
}
