import { ipcMain } from "electron";

import { SERVERS_CALL } from "../shared/ipc";
import type { ProfilesResult } from "../shared/ipc";
import { Preference, settingsDatabase } from "./database";

interface StoredServer {
  id: string;
  name: string;
  url: string;
  secret: string;
}

interface StoredServersState {
  servers: StoredServer[];
  activeId: string | null;
}

interface ServerRow {
  id: string;
  name: string;
  url: string;
  secret: string;
}

const activeServerPreference = new Preference<string | null>(
  "active_remote_server_id",
  null,
  (value) => {
    if (typeof value !== "string" || value === "") {
      throw new Error("invalid active remote server preference");
    }
    return value;
  },
);

function parseServersState(value: unknown): StoredServersState {
  if (typeof value !== "object" || value === null) {
    throw new Error("invalid server storage");
  }
  const candidate = value as { servers?: unknown; activeId?: unknown };
  if (
    !Array.isArray(candidate.servers) ||
    (candidate.activeId !== null && typeof candidate.activeId !== "string")
  ) {
    throw new Error("invalid server storage");
  }
  const servers: StoredServer[] = [];
  const identifiers = new Set<string>();
  for (const entry of candidate.servers) {
    const server = entry as Record<string, unknown> | null;
    if (
      typeof server !== "object" ||
      server === null ||
      typeof server.id !== "string" ||
      server.id === "" ||
      identifiers.has(server.id) ||
      typeof server.name !== "string" ||
      typeof server.url !== "string" ||
      server.url === "" ||
      typeof server.secret !== "string"
    ) {
      throw new Error("invalid server storage");
    }
    identifiers.add(server.id);
    servers.push({
      id: server.id,
      name: server.name,
      url: server.url,
      secret: server.secret,
    });
  }
  if (candidate.activeId !== null && !identifiers.has(candidate.activeId)) {
    throw new Error("invalid server storage");
  }
  return { servers, activeId: candidate.activeId };
}

function loadServers(): StoredServersState {
  const rows = settingsDatabase()
    .prepare(
      "SELECT id, name, url, secret FROM remote_servers ORDER BY item_order ASC",
    )
    .all() as unknown as ServerRow[];
  const servers = rows.map((row) => ({
    id: row.id,
    name: row.name,
    url: row.url,
    secret: row.secret,
  }));
  const activeId = activeServerPreference.get();
  if (activeId !== null && !servers.some((server) => server.id === activeId)) {
    throw new Error("active remote server does not exist");
  }
  return { servers, activeId };
}

function saveServers(value: unknown): void {
  const state = parseServersState(value);
  const rows = state.servers.map((server, itemOrder) => ({
    ...server,
    itemOrder,
  }));
  const store = settingsDatabase();
  store.transaction(() => {
    store.prepare("DELETE FROM remote_servers").run();
    const insert = store.prepare(
      `INSERT INTO remote_servers (id, name, url, secret, item_order)
       VALUES (?, ?, ?, ?, ?)`,
    );
    for (const row of rows) {
      insert.run(row.id, row.name, row.url, row.secret, row.itemOrder);
    }
    activeServerPreference.set(state.activeId);
  })();
}

export function registerServers(): void {
  ipcMain.handle(
    SERVERS_CALL,
    async (
      _event,
      method: string,
      ...callArguments: unknown[]
    ): Promise<ProfilesResult> => {
      try {
        switch (method) {
          case "load":
            return { ok: true, value: loadServers() };
          case "save":
            saveServers(callArguments[0]);
            return { ok: true, value: undefined };
          default:
            return { ok: false, error: `unknown servers method: ${method}` };
        }
      } catch (error) {
        return {
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        };
      }
    },
  );
}
