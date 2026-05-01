/**
 * Centralized error codes for the application.
 * All error codes should be defined here with their default messages.
 * 
 * Benefits:
 * - Single source of truth for all error codes
 * - Prevents typos and inconsistent error codes
 * - Makes it easy to see all possible errors in the app
 * - Default messages can be overridden when creating error results
 * 
 * @example
 * ```ts
 * import { AppErrorCode, getErrorMessage } from '@/lib/errorCodes';
 * 
 * // Use with createErrorResult
 * return createErrorResult(AppErrorCode.USER_NOT_FOUND);
 * 
 * // Or with custom message
 * return createErrorResult(
 *   AppErrorCode.USER_NOT_FOUND,
 *   `User with ID ${userId} not found`
 * );
 * ```
 */

/**
 * Application-wide error codes.
 * Add new error codes here as your application grows.
 */
export enum AppErrorCode {
  // Generic errors
  UNKNOWN_ERROR = 'UNKNOWN_ERROR',
  INTERNAL_ERROR = 'INTERNAL_ERROR',
  
  // Validation errors
  INVALID_INPUT = 'INVALID_INPUT',
  INVALID_EMAIL = 'INVALID_EMAIL',
  INVALID_PASSWORD = 'INVALID_PASSWORD',
  INVALID_ID = 'INVALID_ID',
  MISSING_REQUIRED_FIELD = 'MISSING_REQUIRED_FIELD',
  
  // Authentication & Authorization
  UNAUTHORIZED = 'UNAUTHORIZED',
  FORBIDDEN = 'FORBIDDEN',
  TOKEN_EXPIRED = 'TOKEN_EXPIRED',
  INVALID_CREDENTIALS = 'INVALID_CREDENTIALS',
  
  // Resource errors
  NOT_FOUND = 'NOT_FOUND',
  ALREADY_EXISTS = 'ALREADY_EXISTS',
  CONFLICT = 'CONFLICT',
  
  // Database errors
  DATABASE_ERROR = 'DATABASE_ERROR',
  QUERY_FAILED = 'QUERY_FAILED',
  CONNECTION_ERROR = 'CONNECTION_ERROR',
  
  // External service errors
  EXTERNAL_SERVICE_ERROR = 'EXTERNAL_SERVICE_ERROR',
  API_ERROR = 'API_ERROR',
  NETWORK_ERROR = 'NETWORK_ERROR',
  TIMEOUT = 'TIMEOUT',
  
  // Business logic errors
  INSUFFICIENT_PERMISSIONS = 'INSUFFICIENT_PERMISSIONS',
  OPERATION_NOT_ALLOWED = 'OPERATION_NOT_ALLOWED',
  RATE_LIMIT_EXCEEDED = 'RATE_LIMIT_EXCEEDED',
}

/**
 * Default error messages for each error code.
 * These can be overridden when creating error results.
 */
const ERROR_MESSAGES: Record<AppErrorCode, string> = {
  // Generic errors
  [AppErrorCode.UNKNOWN_ERROR]: 'An unknown error occurred',
  [AppErrorCode.INTERNAL_ERROR]: 'An internal error occurred',
  
  // Validation errors
  [AppErrorCode.INVALID_INPUT]: 'Invalid input provided',
  [AppErrorCode.INVALID_EMAIL]: 'Invalid email address',
  [AppErrorCode.INVALID_PASSWORD]: 'Invalid password',
  [AppErrorCode.INVALID_ID]: 'Invalid ID provided',
  [AppErrorCode.MISSING_REQUIRED_FIELD]: 'Required field is missing',
  
  // Authentication & Authorization
  [AppErrorCode.UNAUTHORIZED]: 'Unauthorized access',
  [AppErrorCode.FORBIDDEN]: 'Access forbidden',
  [AppErrorCode.TOKEN_EXPIRED]: 'Authentication token expired',
  [AppErrorCode.INVALID_CREDENTIALS]: 'Invalid credentials',
  
  // Resource errors
  [AppErrorCode.NOT_FOUND]: 'Resource not found',
  [AppErrorCode.ALREADY_EXISTS]: 'Resource already exists',
  [AppErrorCode.CONFLICT]: 'Resource conflict',
  
  // Database errors
  [AppErrorCode.DATABASE_ERROR]: 'Database error occurred',
  [AppErrorCode.QUERY_FAILED]: 'Database query failed',
  [AppErrorCode.CONNECTION_ERROR]: 'Database connection error',
  
  // External service errors
  [AppErrorCode.EXTERNAL_SERVICE_ERROR]: 'External service error',
  [AppErrorCode.API_ERROR]: 'API request failed',
  [AppErrorCode.NETWORK_ERROR]: 'Network error occurred',
  [AppErrorCode.TIMEOUT]: 'Request timeout',
  
  // Business logic errors
  [AppErrorCode.INSUFFICIENT_PERMISSIONS]: 'Insufficient permissions',
  [AppErrorCode.OPERATION_NOT_ALLOWED]: 'Operation not allowed',
  [AppErrorCode.RATE_LIMIT_EXCEEDED]: 'Rate limit exceeded',
};

/**
 * Gets the default error message for an error code.
 * 
 * @param code - The error code
 * @returns The default error message for the code
 * 
 * @example
 * ```ts
 * const message = getErrorMessage(AppErrorCode.NOT_FOUND);
 * console.log(message); // "Resource not found"
 * ```
 */
export function getErrorMessage(code: AppErrorCode): string {
  return ERROR_MESSAGES[code] ?? ERROR_MESSAGES[AppErrorCode.UNKNOWN_ERROR];
}

/**
 * Type alias for AppErrorCode for convenience.
 * Use this when you want to accept any app error code.
 */
export type TAppErrorCode = AppErrorCode;