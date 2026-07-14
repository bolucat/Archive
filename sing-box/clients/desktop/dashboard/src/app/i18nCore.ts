import { loadStoredString, removeStoredValue, saveStoredString } from "../lib/storage";
import { LANGUAGES, type Language } from "./translations";

export type LanguagePreference = "auto" | Language;

const LANGUAGE_KEY = "language";

export function loadLanguagePreference(): LanguagePreference {
  const value = loadStoredString(LANGUAGE_KEY);
  if (value && LANGUAGES.some((language) => language.value === value)) {
    return value as Language;
  }
  return "auto";
}

export function saveLanguagePreference(preference: LanguagePreference): void {
  if (preference === "auto") {
    removeStoredValue(LANGUAGE_KEY);
  } else {
    saveStoredString(LANGUAGE_KEY, preference);
  }
}

export function detectSystemLanguage(): Language {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const lower = tag.toLowerCase();
    if (lower.startsWith("zh")) {
      return /hant|tw|hk|mo/.test(lower) ? "zh-Hant" : "zh-Hans";
    }
    if (lower.startsWith("fa")) {
      return "fa";
    }
    if (lower.startsWith("ru")) {
      return "ru";
    }
    if (lower.startsWith("en")) {
      return "en";
    }
  }
  return "en";
}

export function subscribeSystemLanguage(onChange: () => void): () => void {
  window.addEventListener("languagechange", onChange);
  return () => window.removeEventListener("languagechange", onChange);
}
