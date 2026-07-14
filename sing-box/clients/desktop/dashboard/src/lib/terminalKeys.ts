// Key ordering and encoding match sing-box-for-apple's TerminalInputAccessoryView.

const ESC = "\x1b";

export type ModState = "off" | "armed" | "locked";

export interface Modifiers {
  ctrl: ModState;
  alt: ModState;
}

export type ModKey = keyof Modifiers;

export type SpecialKeyId = "esc" | "tab" | "up" | "down" | "left" | "right";

export type TerminalKey =
  | { kind: "modifier"; mod: ModKey; label: string }
  | { kind: "special"; id: SpecialKeyId; label: string }
  | { kind: "text"; char: string }
  | { kind: "paste" }
  | { kind: "divider" };

export const DEFAULT_KEYS: readonly (TerminalKey & { key: string })[] = [
  { key: "escape", kind: "special", id: "esc", label: "esc" },
  { key: "tab", kind: "special", id: "tab", label: "tab" },
  { key: "control", kind: "modifier", mod: "ctrl", label: "⌃" },
  { key: "option", kind: "modifier", mod: "alt", label: "⌥" },
  { key: "navigation-divider", kind: "divider" },
  { key: "left", kind: "special", id: "left", label: "←" },
  { key: "up", kind: "special", id: "up", label: "↑" },
  { key: "down", kind: "special", id: "down", label: "↓" },
  { key: "right", kind: "special", id: "right", label: "→" },
  { key: "symbols-divider", kind: "divider" },
  { key: "pipe", kind: "text", char: "|" },
  { key: "slash", kind: "text", char: "/" },
  { key: "tilde", kind: "text", char: "~" },
  { key: "hyphen", kind: "text", char: "-" },
  { key: "underscore", kind: "text", char: "_" },
  { key: "backtick", kind: "text", char: "`" },
  { key: "single-quote", kind: "text", char: "'" },
  { key: "double-quote", kind: "text", char: '"' },
  { key: "paste", kind: "paste" },
];

function controlByte(ch: string): string | null {
  if (ch.length !== 1) {
    return null;
  }
  if (ch === " ") {
    return "\x00";
  }
  const code = ch.toUpperCase().charCodeAt(0);
  if (code >= 0x40 && code <= 0x5f) {
    return String.fromCharCode(code & 0x1f);
  }
  return null;
}

export function encodeText(text: string, mods: Modifiers): string {
  let out = text;
  if (mods.ctrl !== "off" && text.length === 1) {
    const byte = controlByte(text);
    if (byte !== null) {
      out = byte;
    }
  }
  if (mods.alt !== "off") {
    out = ESC + out;
  }
  return out;
}

const ARROW_FINAL: Record<"up" | "down" | "left" | "right", string> = {
  up: "A",
  down: "B",
  right: "C",
  left: "D",
};

export function encodeSpecial(id: SpecialKeyId, mods: Modifiers): string {
  if (id === "esc") {
    return mods.alt !== "off" ? ESC + ESC : ESC;
  }
  if (id === "tab") {
    return mods.alt !== "off" ? ESC + "\t" : "\t";
  }
  const final = ARROW_FINAL[id];
  const modCode = 1 + (mods.alt !== "off" ? 2 : 0) + (mods.ctrl !== "off" ? 4 : 0);
  return modCode === 1 ? ESC + "[" + final : ESC + "[1;" + modCode + final;
}

export function hasActiveModifier(mods: Modifiers): boolean {
  return mods.ctrl !== "off" || mods.alt !== "off";
}

export function armModifier(mods: Modifiers, which: ModKey, doubleTap: boolean): Modifiers {
  let next: ModState;
  switch (mods[which]) {
    case "off":
      next = "armed";
      break;
    case "armed":
      next = doubleTap ? "locked" : "off";
      break;
    case "locked":
      next = "off";
      break;
  }
  return { ...mods, [which]: next };
}

export function consumeArmed(mods: Modifiers): Modifiers {
  return {
    ctrl: mods.ctrl === "armed" ? "off" : mods.ctrl,
    alt: mods.alt === "armed" ? "off" : mods.alt,
  };
}
