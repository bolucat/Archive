import { loadStoredJson, saveStoredJson } from "../lib/storage";

const DISABLE_DEPRECATED_WARNINGS_KEY = "disable-deprecated-warnings";

export function loadDisableDeprecatedWarnings(): boolean {
  return loadStoredJson(DISABLE_DEPRECATED_WARNINGS_KEY) === true;
}

export function saveDisableDeprecatedWarnings(value: boolean): void {
  saveStoredJson(DISABLE_DEPRECATED_WARNINGS_KEY, value);
}
