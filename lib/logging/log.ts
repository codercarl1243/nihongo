/**
 * Structured logging utility for Next.js / TypeScript applications.
 * 
 * Provides type-safe logging with different severity levels and structured output.
 * All logs include timestamps and can be enriched with context and additional data.
 * 
 * @example
 * ```ts
 * // Simple info log
 * log("User logged in successfully");
 * 
 * // Warning with context
 * log("Missing optional field in request", "warning", { 
 *   context: "SignupForm",
 *   data: { field: "phoneNumber" }
 * });
 * 
 * // Error with stack trace (error parameter is required)
 * try {
 *   await db.connect();
 * } catch (err) {
 *   log("Database connection failed", "error", err, { 
 *     context: "DBService",
 *     trace: true
 *   });
 * }
 * ```
 */

import type {
  TErrorLogOptions,
  TLogOptions,
  TLogLevel,
  TErrorLogEntry,
  TWarningLogEntry,
  TDefaultLogEntry,
} from "./log.type";

/* eslint-disable no-redeclare */

// Overload signatures
export function log(
  message: string,
  level: "error",
  error: unknown,
  options?: TErrorLogOptions
): void;
export function log(
  message: string,
  level: "warning",
  options?: TLogOptions
): void;
export function log(
  message: string,
  level?: "default",
  options?: TLogOptions
): void;
export function log(
  message: string,
  levelOrOptions?: TLogLevel,
  errorOrOptions?: unknown,
  finalOptions?: TErrorLogOptions
): void {
  const level: TLogLevel = levelOrOptions || "default";
  let error: unknown;
  let options: TErrorLogOptions = {};

  if (level === "error") {
    error = errorOrOptions;
    options = finalOptions || {};
  } else {
    options = (errorOrOptions as TLogOptions) || {};
  }

  const { context, data, trace } = options;
  const timestamp = new Date().toISOString();

  if (level === "error") {
    let errorMsg = "";
    let stack: string | undefined;

    if (error instanceof Error) {
      errorMsg = error.message;
      stack = trace ? error.stack : undefined;
    } else if (typeof error === "string") {
      errorMsg = error;
    } else if (typeof error === "object" && error !== null) {
      try {
        errorMsg = safeToString(error);
      } catch {
        errorMsg = "[Unserializable Object]";
      }
    } else if (error !== undefined) {
      errorMsg = safeToString(error);
    }

    const entry: TErrorLogEntry = {
      level: "error",
      message,
      error: errorMsg,
      stack,
      context,
      data,
      timestamp,
    };

    // eslint-disable-next-line no-console
    console.error(`[ERROR]`, entry);
    return;
  }

  if (level === "warning") {
    const entry: TWarningLogEntry = {
      level: "warning",
      message,
      context,
      data,
      timestamp,
    };
    // eslint-disable-next-line no-console
    console.warn(`[WARN]`, entry);
    return;
  }

  const entry: TDefaultLogEntry = {
    level: "default",
    message,
    context,
    data,
    timestamp,
  };
  // eslint-disable-next-line no-console
  console.log(`[LOG]`, entry);
}

export default log;
/* eslint-enable no-redeclare */

/**
 * Convenience function for logging informational messages.
 * Equivalent to calling log() with level 'default'.
 * 
 * @param message - Human-readable message
 * @param options - Optional context and data
 * 
 * @example
 * ```ts
 * logInfo("User session started", {
 *   context: "AuthService",
 *   data: { userId: "123" }
 * });
 * ```
 */
export function logInfo(
  message: string,
  options?: TLogOptions
): void {
    log(message, 'default', options);
}

/**
 * Convenience function for logging warnings.
 * Equivalent to calling log() with level 'warning'.
 * 
 * @param message - Human-readable warning message
 * @param options - Optional context and data
 * 
 * @example
 * ```ts
 * logWarning("Deprecated API endpoint used", {
 *   context: "ApiRouter",
 *   data: { endpoint: "/api/v1/users" }
 * });
 * ```
 */
export function logWarning(
  message: string,
  options?: TLogOptions
): void {
  log(message, 'warning', options);
}

/**
 * Convenience function for logging errors.
 * Equivalent to calling log() with level 'error'.
 * 
 * @param message - Human-readable error message
 * @param error - Error object, string, or any value to log
 * @param options - Optional context, data, and trace flag
 * 
 * @example
 * ```ts
 * try {
 *   await db.user.delete({ where: { id } });
 * } catch (err) {
 *   logError("Failed to delete user", err, {
 *     context: "UserService",
 *     data: { userId: id },
 *     trace: true
 *   });
 * }
 * ```
 */
export function logError(
  message: string,
  error: unknown,
  options?: TErrorLogOptions
): void {
  log(message, 'error', error, options);
}

function safeToString(value: unknown): string {
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}