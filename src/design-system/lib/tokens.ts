export function isOneOfTokens<T extends readonly string[]>(
  tokens: T,
  value: unknown
): value is T[number] {
  return typeof value === 'string' && tokens.includes(value);
}