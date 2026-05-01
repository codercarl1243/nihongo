import type { KeyboardEvent as ReactKeyBoardEvent } from 'react'

export type KeyPressEventType = ReactKeyBoardEvent | KeyboardEvent;

type Modifier = 'control' | 'meta' | 'shift' | 'alt';

type Alphabet = 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g' | 'h' | 'i' | 'j' | 'k' | 'l' | 'm' | 'n' | 'o' | 'p' | 'q' | 'r' | 's' | 't' | 'u' | 'v' | 'w' | 'x' | 'y' | 'z';

type SpecialKey =
  | 'enter' | 'escape' | 'space' | 'tab' | 'backspace' | 'delete'
  | 'arrowup' | 'arrowdown' | 'arrowleft' | 'arrowright'
  | 'home' | 'end' | 'pageup' | 'pagedown'
  | '/' | '?' | '[' | ']' | '\\' | '=' | '+' | '-' | '_' | '.' | ',' | ';' | "'" | '`';

type BaseKey = Alphabet | SpecialKey | (string & {});

export type KeyTrigger =
  | BaseKey
  | `${Modifier}+${BaseKey}`
  | `${Modifier}+${Modifier}+${BaseKey}`
  | `${Modifier}+${Modifier}+${Modifier}+${BaseKey}`;

export type KeyPressCallbackMap = {
  [K in KeyTrigger]?: (e: KeyPressEventType) => void;
};