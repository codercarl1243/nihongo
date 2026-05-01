import { AppErrorCode, getErrorMessage } from './errorCodes';
import { logError } from './log';

/**
 * Logs a structured application error using a known AppErrorCode.
 * 
 * Provides consistent formatting and makes error intent self-documenting.
 * 
 * @example
 * ```ts
 * logAppError(AppErrorCode.INVALID_INPUT);
 * logAppError(AppErrorCode.NOT_FOUND, "Post not found", {
 *   context: "getPostBySlug",
 *   data: { slug }
 * });
 * ```
 */
export function logAppError(
    code: AppErrorCode,
    customMessage?: string,
    options?: {
        context?: string;
        data?: Record<string, unknown>;
        trace?: boolean;
    }
): void {
    const { context, data: dataObj = {}, trace } = options ?? {};
    const message = customMessage ?? getErrorMessage(code);

    logError(message, undefined, {
        context,
        trace,
        data: {
            code,
            ...dataObj
        },
    });
}