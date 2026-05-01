/**
 * Type-safe Result pattern for handling operations that can succeed or fail.
 * 
 * @example
 * ```ts
 * type UserError = 'NOT_FOUND' | 'INVALID_ID';
 * 
 * async function getUser(id: string): Promise<TResult<User, UserError>> {
 *   if (!id) return createErrorResult('INVALID_ID', 'User ID is required');
 *   const user = await db.user.findUnique({ where: { id } });
 *   if (!user) return createErrorResult('NOT_FOUND', 'User not found');
 *   return createSuccessfulResult(user);
 * }
 * 
 * const result = await getUser('123');
 * if (isError(result)) {
 *   console.error(result.error.code); // Type-safe error handling
 * } else {
 *   console.log(result.result); // Type-safe success handling
 * }
 * ```
 */

import { AppErrorCode, getErrorMessage } from "./logging/errorCodes";
import { logAppError } from "./logging/logAppError";
import { isDefined, isNonEmptyObject, isNonEmptyString, isUndefined } from "./guards";
import { TErrorCode } from "./logging/log.type";
import { tryCatch, tryCatchAsync } from "./trycatch";

/**
 * Represents the outcome of an operation that can either succeed or fail.
 * @template Result - The type of the successful result value
 * @template TErrorCode - The type of error codes (typically a union of string literals)
 */
export type TResult<Result, TErrorCode> = TSuccessfulResult<Result> | TErrorResult<TErrorCode>;

/**
 * Represents a successful operation result.
 * @template Result - The type of the result value
 */
export type TSuccessfulResult<Result> = {
    result: Result;
    error?: undefined;
};

/**
 * Represents a failed operation result.
 * @template TErrorCode - The type of error codes
 */
export type TErrorResult<TErrorCode> = {
    result?: never;
    error: TError<TErrorCode>;
};

/**
 * Error object containing a typed error code and descriptive message.
 * @template TErrorCode - The type of error codes
 */
export type TError<TErrorCode> = {
    code: TErrorCode;
    message: string;
};

/**
 * Creates a typed error result.
 * 
 * @param code - The error code (refer to AppErrorCode enum)
 * @param message - Optional human-readable error message. Defaults to "DEFAULT_ERROR_MESSAGE"
 * @returns A typed error result
 * 
 * @example
 * ```ts
 * type AuthError = 'UNAUTHORIZED' | 'EXPIRED_TOKEN';
 * return createErrorResult<AuthError>('UNAUTHORIZED', 'Please log in');
 * ```
 */

export function createErrorResult(
    errorCode: TErrorCode,
    message?: string
): TErrorResult<TErrorCode> {
    if (!errorCode) {
        return {
            error: {
                code: AppErrorCode.INTERNAL_ERROR,
                message: "Error code cannot be empty or falsy",
            },
        };
    }

    const defaultMessage =
        Object.values(AppErrorCode).includes(errorCode as AppErrorCode)
            ? getErrorMessage(errorCode as AppErrorCode)
            : "DEFAULT_ERROR_MESSAGE";

    const finalMessage = isNonEmptyString(message) ? message : defaultMessage

    return {
        error: {
            code: errorCode,
            message: finalMessage
        },
    };
}


/**
 * Creates a successful result containing a value.
 * 
 * @param result - The successful result value
 * @returns A typed successful result
 * 
 * @example
 * ```ts
 * const user = await db.user.findUnique({ where: { id } });
 * return createSuccessfulResult(user);
 * ```
 */
export function createSuccessfulResult<T>(result: T): TSuccessfulResult<T> {
    return {
        result,
    };
}

/**
 * Creates a successful result for operations that don't return a value.
 * Useful for operations like delete, update, or void-returning functions.
 * 
 * @returns A successful result with void type
 * 
 * @example
 * ```ts
 * async function deleteUser(id: string): Promise<TResult<void, DbError>> {
 *   await db.user.delete({ where: { id } });
 *   return createVoidResult();
 * }
 * ```
 */
export function createVoidResult(): TSuccessfulResult<void> {
    return {
        result: undefined,
    };
}

/** Narrows the type to TErrorResult, making error properties accessible. */
export function isError<Result>(
    result: TResult<Result, TErrorCode>
): result is TErrorResult<TErrorCode> {
    return isDefined(result.error);
}

/** Narrows the type to TSuccessfulResult, making the result value accessible. */
export function isSuccess<Result>(
    result: TResult<Result, TErrorCode>
): result is TSuccessfulResult<Result> {
    return isUndefined(result.error);
}


type TPipeResult<TErrorCode> =
    | TResult<unknown, TErrorCode>
    | TErrorResult<AppErrorCode>;

/**
 * Chains multiple async operations that return Results.
 * Executes operations sequentially, passing the result of each to the next.
 * Stops and returns the first error encountered.
 * 
 * Note: Thrown errors (exceptions) are NOT caught and will propagate as
 * promise rejections. Operations should return error Results instead of throwing.
 * 
 * @param initialValue - The starting value to pipe through the operations
 * @param fns - Array of async functions that take a value and return a Result
 * @returns A Result containing the final value or the first error encountered
 * 
 * @example
 * ```ts
 * // Clean sequential operations
 * const result = await pipe(
 *   userId,
 *   getUser,                          // User ID → Result<User>
 *   (user) => getUserSettings(user.id), // User → Result<Settings>
 *   validateSettings,                  // Settings → Result<Settings>
 *   updateSettings                     // validated Settings → Result<Settings>
 * );
 * ```
 */

/* eslint-disable no-redeclare */
export async function pipeAsync<T, U>(
    initialValue: T,
    fn1: (value: T) => Promise<TResult<U, TErrorCode>>
): Promise<TPipeResult<TErrorCode>>;

export async function pipeAsync<T, U, V>(
    initialValue: T,
    fn1: (value: T) => Promise<TResult<U, TErrorCode>>,
    fn2: (value: U) => Promise<TResult<V, TErrorCode>>
): Promise<TPipeResult<TErrorCode>>;

export async function pipeAsync<T, U, V, W>(
    initialValue: T,
    fn1: (value: T) => Promise<TResult<U, TErrorCode>>,
    fn2: (value: U) => Promise<TResult<V, TErrorCode>>,
    fn3: (value: V) => Promise<TResult<W, TErrorCode>>
): Promise<TPipeResult<TErrorCode>>;

export async function pipeAsync<T, U, V, W, X>(
    initialValue: T,
    fn1: (value: T) => Promise<TResult<U, TErrorCode>>,
    fn2: (value: U) => Promise<TResult<V, TErrorCode>>,
    fn3: (value: V) => Promise<TResult<W, TErrorCode>>,
    fn4: (value: W) => Promise<TResult<X, TErrorCode>>
): Promise<TPipeResult<TErrorCode>>;

export async function pipeAsync<T, U, V, W, X, Y>(
    initialValue: T,
    fn1: (value: T) => Promise<TResult<U, TErrorCode>>,
    fn2: (value: U) => Promise<TResult<V, TErrorCode>>,
    fn3: (value: V) => Promise<TResult<W, TErrorCode>>,
    fn4: (value: W) => Promise<TResult<X, TErrorCode>>,
    fn5: (value: X) => Promise<TResult<Y, TErrorCode>>
): Promise<TPipeResult<TErrorCode>>;


export async function pipeAsync<T>(
    initialValue: T,
    ...fns: Array<(value: unknown) => Promise<TResult<unknown, TErrorCode>>>
): Promise<TPipeResult<TErrorCode>> {

    let current: TPipeResult<TErrorCode> = createSuccessfulResult(initialValue);
    for (let i = 0; i < fns.length; i++) {
        if (current.error) {
            // Log the error with context about where it occurred
            logAppError(
                current.error.code as AppErrorCode,
                current.error.message,
                { context: `pipeAsync step ${i}` }
            );
            return current;
        }

        // Use tryCatchAsync to handle thrown errors
        const result = await tryCatchAsync(
            () => fns[i](current.result),
            AppErrorCode.INTERNAL_ERROR,
            {
                context: `pipeAsync step ${i + 1}`,
                errorMessage: 'Function threw an error instead of returning a Result',
                trace: true
            }
        );

        if (isError(result)) {
            return result;
        }

        current = result.result;

        const isValidResult =
            isNonEmptyObject(current) &&
            ("error" in current || "result" in current);

        if (!isValidResult) {
            const errorMsg = "Function did not return a valid Result object";
            logAppError(
                AppErrorCode.INTERNAL_ERROR,
                errorMsg,
                { context: `pipeAsync step ${i + 1} (invalid return)` }
            );

            return createErrorResult(
                AppErrorCode.INTERNAL_ERROR,
                errorMsg
            );
        }
    }

    return current;
}
/* eslint-enable no-redeclare */


/* eslint-disable no-redeclare */
export function pipe<T, U>(
    initialValue: T,
    fn1: (value: T) => TResult<U, TErrorCode>
): TPipeResult<TErrorCode>;

export function pipe<T, U, V>(
    initialValue: T,
    fn1: (value: T) => TResult<U, TErrorCode>,
    fn2: (value: U) => TResult<V, TErrorCode>
): TPipeResult<TErrorCode>;

export function pipe<T, U, V, W>(
    initialValue: T,
    fn1: (value: T) => TResult<U, TErrorCode>,
    fn2: (value: U) => TResult<V, TErrorCode>,
    fn3: (value: V) => TResult<W, TErrorCode>
): TPipeResult<TErrorCode>;

export function pipe<T, U, V, W, X>(
    initialValue: T,
    fn1: (value: T) => TResult<U, TErrorCode>,
    fn2: (value: U) => TResult<V, TErrorCode>,
    fn3: (value: V) => TResult<W, TErrorCode>,
    fn4: (value: W) => TResult<X, TErrorCode>
): TPipeResult<TErrorCode>;

export function pipe<T, U, V, W, X, Y>(
    initialValue: T,
    fn1: (value: T) => TResult<U, TErrorCode>,
    fn2: (value: U) => TResult<V, TErrorCode>,
    fn3: (value: V) => TResult<W, TErrorCode>,
    fn4: (value: W) => TResult<X, TErrorCode>,
    fn5: (value: X) => TResult<Y, TErrorCode>
): TPipeResult<TErrorCode>;

export function pipe<T, U, V, W, X, Y, Z>(
    initialValue: T,
    fn1: (value: T) => TResult<U, TErrorCode>,
    fn2: (value: U) => TResult<V, TErrorCode>,
    fn3: (value: V) => TResult<W, TErrorCode>,
    fn4: (value: W) => TResult<X, TErrorCode>,
    fn5: (value: X) => TResult<Y, TErrorCode>,
    fn6: (value: Y) => TResult<Z, TErrorCode>
): TPipeResult<TErrorCode>;

export function pipe<T>(
    initialValue: T,
    ...fns: Array<(value: unknown) => TResult<unknown, TErrorCode>>
): TPipeResult<TErrorCode>;

export function pipe<T>(
    initialValue: T,
    ...fns: Array<(value: unknown) => TResult<unknown, TErrorCode>>
): TPipeResult<TErrorCode> {
    let current: TPipeResult<TErrorCode> = createSuccessfulResult(initialValue);

    for (let i = 0; i < fns.length; i++) {
        if (current.error) {
            // Log the error with context about where it occurred
            logAppError(
                current.error.code as AppErrorCode,
                current.error.message,
                { context: `pipe step ${i}` }
            );
            return current;
        }

        // Use tryCatchAsync to handle thrown errors
        const result = tryCatch(
            () => fns[i](current.result),
            AppErrorCode.INTERNAL_ERROR,
            {
                context: `pipe step ${i + 1}`,
                errorMessage: 'Function threw an error instead of returning a Result',
                trace: true
            }
        );

        if (isError(result)) {
            return result;
        }

        current = result.result;

        const isValidResult =
            isNonEmptyObject(current) &&
            ("error" in current || "result" in current);

        if (!isValidResult) {
            const errorMsg = "Function did not return a valid Result object";
            logAppError(
                AppErrorCode.INTERNAL_ERROR,
                errorMsg,
                { context: `pipe step ${i + 1} (invalid return)` }
            );

            return createErrorResult(
                AppErrorCode.INTERNAL_ERROR,
                errorMsg
            );
        }
    }

    return current;
}
/* eslint-enable no-redeclare */