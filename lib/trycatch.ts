import { logError } from "./logging/log";
import type { TErrorCode, TErrorLogOptions, TLogOptions } from "./logging/log.type";
import { type TResult, createSuccessfulResult, createErrorResult } from "./results";

// TODO: add optional function that runs inside of the catch block

/**
 * Wraps an async function that might throw, converting exceptions to typed Results.
 * Automatically logs any errors that occur.
 * 
 * @template Result - The type of the successful result
 * @template ErrorCode - The type of error codes
 * @param fn - Async function to execute (typically a Promise-returning operation)
 * @param errorCode - The error code to return if the function throws
 * @param options - Optional configuration
 * @param options.context - Where this operation occurs (e.g., "DatabaseService")
 * @param options.errorMessage - Custom error message (defaults to "Operation failed")
 * @param options.trace - Whether to include stack trace in logs
 * @returns A Result containing either the successful value or a typed error
 * 
 * @example
 * ```ts
 * type DbError = 'CONNECTION_ERROR' | 'NOT_FOUND' | 'QUERY_ERROR';
 * 
 * const userResult = await tryCatchAsync(
 *   () => db.user.findUnique({ where: { id: userId } }),
 *   'QUERY_ERROR',
 *   { 
 *     context: 'UserRepository',
 *     errorMessage: 'Failed to fetch user from database',
 *     trace: true
 *   }
 * );
 * 
 * if (isError(userResult)) {
 *   // Error was automatically logged with context
 *   return userResult;
 * }
 * 
 * const user = userResult.result;
 * ```
 */
export async function tryCatchAsync<Result>(
    fn: () => Promise<Result>,
    errorCode: TErrorCode,
    options?: {
        context?: TLogOptions['context'];
        errorMessage?: string;
        trace?: TErrorLogOptions['trace'];
    }
): Promise<TResult<Result, TErrorCode>> {
    try {
        const result = await fn();
        return createSuccessfulResult(result);
    } catch (error) {
        const message = options?.errorMessage ?? 'Operation failed';
        logError(message, error, {
            context: options?.context,
            trace: options?.trace,
        });
        return createErrorResult(errorCode, message);
    }
}

/**
 * Synchronous version of tryCatchAsync.
 * Wraps a synchronous function that might throw, converting exceptions to typed Results.
 * Automatically logs any errors that occur.
 * 
 * @template Result - The type of the successful result
 * @template ErrorCode - The type of error codes
 * @param fn - Synchronous function to execute
 * @param errorCode - The error code to return if the function throws
 * @param options - Optional configuration
 * @param options.context - Where this operation occurs
 * @param options.errorMessage - Custom error message (defaults to "Operation failed")
 * @param options.trace - Whether to include stack trace in logs
 * @returns A Result containing either the successful value or a typed error
 * 
 * @example
 * ```ts
 * type ParseError = 'INVALID_JSON' | 'MISSING_FIELD';
 * 
 * const configResult = tryCatch(
 *   () => JSON.parse(configString),
 *   'INVALID_JSON',
 *   { 
 *     context: 'ConfigLoader',
 *     errorMessage: 'Failed to parse configuration file'
 *   }
 * );
 * 
 * if (isError(configResult)) {
 *   return defaultConfig;
 * }
 * 
 * return configResult.result;
 * ```
 */
export function tryCatch<Result>(
    fn: () => Result,
    errorCode: TErrorCode,
    options?: {
        context?: TLogOptions['context'];
        errorMessage?: string;
        trace?: TErrorLogOptions['trace'];
    }
): TResult<Result, TErrorCode> {
    try {
        const result = fn();
        return createSuccessfulResult(result);
    } catch (error) {
        const message = options?.errorMessage ?? 'Operation failed';
        logError(message, error, {
            context: options?.context,
            trace: options?.trace,
        });
        return createErrorResult(errorCode, message);
    }
}
