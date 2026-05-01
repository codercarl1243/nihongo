import type { AppErrorCode } from "./errorCodes";

export type TLogLevel = 'default' | 'warning' | 'error';

export type TLogOptions = {
  context?: string;
  data?: Record<string, unknown>;
};

export type TErrorLogOptions = TLogOptions & {
  trace?: boolean;
};

export type TBaseLogEntry = {
  level: TLogLevel;
  message: string;
  context?: string;
  data?: Record<string, unknown>;
  timestamp: string;
};

export type TDefaultLogEntry = TBaseLogEntry & {
  level: 'default';
};

export type TWarningLogEntry = TBaseLogEntry & {
  level: 'warning';
};

export type TErrorLogEntry = TBaseLogEntry & {
  level: 'error';
  error: string;   // required for error logs
  stack?: string;  // optional stack trace
};

export type TLogEntry = TDefaultLogEntry | TWarningLogEntry | TErrorLogEntry;

export type TErrorCode = Exclude<AppErrorCode | string, "" | null | undefined>;