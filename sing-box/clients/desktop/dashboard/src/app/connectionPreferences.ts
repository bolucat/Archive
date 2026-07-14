import { loadStoredJson, saveStoredJson } from "../lib/storage";

const STATE_FILTER_KEY = "connection-state-filter";
const SORT_MODE_KEY = "connection-sort";

export type ConnectionStateFilter = "all" | "active" | "closed";
export type ConnectionSortMode = "date" | "traffic" | "trafficTotal";

const STATE_FILTERS: ConnectionStateFilter[] = ["all", "active", "closed"];
const SORT_MODES: ConnectionSortMode[] = ["date", "traffic", "trafficTotal"];

export function loadConnectionStateFilter(): ConnectionStateFilter {
  const value = loadStoredJson(STATE_FILTER_KEY);
  return STATE_FILTERS.find((filter) => filter === value) ?? "active";
}

export function saveConnectionStateFilter(value: ConnectionStateFilter): void {
  saveStoredJson(STATE_FILTER_KEY, value);
}

export function loadConnectionSortMode(): ConnectionSortMode {
  const value = loadStoredJson(SORT_MODE_KEY);
  return SORT_MODES.find((mode) => mode === value) ?? "date";
}

export function saveConnectionSortMode(value: ConnectionSortMode): void {
  saveStoredJson(SORT_MODE_KEY, value);
}
