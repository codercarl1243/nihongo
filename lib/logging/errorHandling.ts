/**
 * Error handling utilities that combine logging with the Result pattern.
 * 
 * These functions automatically log errors/warnings while returning typed Results,
 * providing a consistent way to handle and track errors throughout your application.
 * 
 * @example
 * ```ts
 * // Wrap risky operations
 * const userResult = await tryCatchAsync(
 *   () => db.user.findUnique({ where: { id } }),
 *   'DATABASE_ERROR',
 *   { context: 'UserService' }
 * );
 * 
 * // Validate input
 * const emailResult = validate(
 *   email,
 *   (e) => e.includes('@'),
 *   'INVALID_EMAIL',
 *   'Email must contain @'
 * );
 * 
 * // Quick error returns with logging
 * if (!userId) {
 *   return logAndReturnError('MISSING_ID', 'User ID is required', { context: 'getUser' });
 * }
 * ```
 */

import { logError, logWarning } from './log';
import type { TErrorLogOptions, TErrorCode } from './log.type';
import { createErrorResult, createSuccessfulResult, TErrorResult, TResult} from '../results';

/**
 * Logs an error and returns a typed error result.
 * Convenience function for common pattern of logging then returning an error.
 * 
 * @template ErrorCode - The type of error codes
 * @param code - The error code to return
 * @param message - Human-readable error message (will be logged and returned)
 * @param options - Optional logging context and additional data
 * @param options.context - Where the error occurred (e.g., "UserService", "AuthMiddleware")
 * @param options.data - Additional debugging data to include in logs
 * @returns A typed error result
 * 
 * @example
 * ```ts
 * type UserError = 'NOT_FOUND' | 'INVALID_ID';
 * 
 * function getUser(id: string): IResult<User, UserError> {
 *   if (!id) {
 *     return logAndReturnError('INVALID_ID', 'User ID is required', { 
 *       context: 'UserService' 
 *     });
 *   }
 *   // ... rest of function
 * }
 * ```
 */
export function logAndReturnError(
    code: TErrorCode,
    message: string,
    options?: TErrorLogOptions
): TErrorResult<TErrorCode> {
    logError(message, undefined, options);
    return createErrorResult(code, message);
}


/**
 * Validates a value against a predicate and returns a typed Result.
 * Logs a warning if validation fails.
 * 
 * @template T - The type of the value being validated
 * @template ErrorCode - The type of error codes
 * @param value - The value to validate
 * @param predicate - Function that returns true if value is valid
 * @param errorCode - Error code to return if validation fails
 * @param errorMessage - Human-readable validation error message
 * @param options - Optional configuration
 * @param options.context - Where this validation occurs (e.g., "SignupForm")
 * @returns A Result containing either the validated value or a typed error
 * 
 * @example
 * ```ts
 * type ValidationError = 'INVALID_EMAIL' | 'INVALID_PASSWORD' | 'INVALID_USERNAME';
 * 
 * // Email validation
 * const emailResult = validate(
 *   email,
 *   (e) => e.includes('@') && e.includes('.'),
 *   'INVALID_EMAIL',
 *   'Email must be valid',
 *   { context: 'SignupForm' }
 * );
 * 
 * // Password validation
 * const passwordResult = validate(
 *   password,
 *   (p) => p.length >= 8,
 *   'INVALID_PASSWORD',
 *   'Password must be at least 8 characters',
 *   { context: 'SignupForm' }
 * );
 * 
 * // Chain validations
 * if (isError(emailResult)) return emailResult;
 * if (isError(passwordResult)) return passwordResult;
 * 
 * // All validations passed
 * await createUser(emailResult.result, passwordResult.result);
 * ```
 */
export function validate<T>(
    value: T,
    predicate: (value: T) => boolean,
    errorCode: TErrorCode,
    errorMessage: string,
    options?: { context?: TErrorLogOptions['context'] }
): TResult<T, TErrorCode> {
    if (!predicate(value)) {
        logWarning(errorMessage, options);
        return createErrorResult(errorCode, errorMessage);
    }
    return createSuccessfulResult(value);
}