import { loadStoredJson, removeStoredValue, saveStoredJson } from "../lib/storage";

export interface Server {
  id: string;
  name: string;
  url: string;
  secret: string;
}

export interface ServersState {
  servers: Server[];
  activeId: string | null;
}

const STORAGE_KEY = "servers";
const LEGACY_STORAGE_KEY = "server";

export function createServerId(): string {
  if (typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export function normalizeServerUrl(url: string): string {
  return url.trim().replace(/\/+$/, "").replace(/^http:\/\//i, "");
}

export function serverConnectUrl(url: string): string {
  const value = url.trim().replace(/\/+$/, "");
  if (value === "") {
    return "";
  }
  return /^https?:\/\//i.test(value) ? value : `http://${value}`;
}

export function serverDisplayName(server: Server): string {
  if (server.name.trim() !== "") {
    return server.name;
  }
  try {
    return new URL(serverConnectUrl(server.url)).host;
  } catch {
    return server.url;
  }
}

function migrateLegacy(): ServersState | null {
  const parsed = loadStoredJson(LEGACY_STORAGE_KEY) as { url?: string; secret?: string } | null;
  removeStoredValue(LEGACY_STORAGE_KEY);
  if (!parsed || typeof parsed.url !== "string" || parsed.url === "") {
    return null;
  }
  const server: Server = {
    id: createServerId(),
    name: "",
    url: normalizeServerUrl(parsed.url),
    secret: typeof parsed.secret === "string" ? parsed.secret : "",
  };
  return { servers: [server], activeId: server.id };
}

export function loadServersState(): ServersState {
  const parsed = loadStoredJson(STORAGE_KEY) as Partial<ServersState> | null;
  if (parsed) {
    const servers = (Array.isArray(parsed.servers) ? parsed.servers : []).filter(
      (server): server is Server =>
        typeof server === "object" &&
        server !== null &&
        typeof server.id === "string" &&
        typeof server.url === "string" &&
        server.url !== "",
    );
    const normalized = servers.map((server) => ({
      ...server,
      name: typeof server.name === "string" ? server.name : "",
      secret: typeof server.secret === "string" ? server.secret : "",
      url: normalizeServerUrl(server.url),
    }));
    const activeId =
      typeof parsed.activeId === "string" && normalized.some((server) => server.id === parsed.activeId)
        ? parsed.activeId
        : (normalized[0]?.id ?? null);
    return { servers: normalized, activeId };
  }
  const migrated = migrateLegacy();
  if (migrated) {
    saveServersState(migrated);
    return migrated;
  }
  return { servers: [], activeId: null };
}

export function saveServersState(state: ServersState) {
  saveStoredJson(STORAGE_KEY, state);
}

export function upsertServer(state: ServersState, server: Server): ServersState {
  const exists = state.servers.some((entry) => entry.id === server.id);
  const servers = exists
    ? state.servers.map((entry) => (entry.id === server.id ? server : entry))
    : [...state.servers, server];
  return { servers, activeId: exists ? state.activeId : (state.activeId ?? server.id) };
}

export function removeServer(state: ServersState, id: string): ServersState {
  const servers = state.servers.filter((entry) => entry.id !== id);
  return {
    servers,
    activeId: state.activeId === id ? (servers[0]?.id ?? null) : state.activeId,
  };
}
