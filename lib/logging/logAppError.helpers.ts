import { AppErrorCode } from "./errorCodes";
import { logAppError } from "./logAppError";

type TLogErrorOptions = {
  context?: string;
  data?: Record<string, unknown>;
  trace?: boolean;
};

export const logValidationError = (
  message?: string,
  options?: TLogErrorOptions
) => logAppError(AppErrorCode.INVALID_INPUT, message, options);

export const logAuthError = (
  message?: string,
  options?: TLogErrorOptions
) => logAppError(AppErrorCode.UNAUTHORIZED, message, options);