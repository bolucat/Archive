import { app, safeStorage } from "electron";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

interface PreferenceRow {
  name: string;
  data: Uint8Array;
}

const MAXIMUM_PREFERENCE_BYTES = 1024 * 1024;
const LEGACY_ENCRYPTED_VALUE_PREFIX = "safe-storage:";

class SettingsDatabase extends DatabaseSync {
  transaction<Arguments extends unknown[], Result>(
    operation: (...args: Arguments) => Result,
  ): (...args: Arguments) => Result {
    return (...args) => {
      this.exec("BEGIN");
      try {
        const result = operation(...args);
        this.exec("COMMIT");
        return result;
      } catch (error) {
        try {
          this.exec("ROLLBACK");
        } catch (rollbackError) {
          throw new AggregateError(
            [error, rollbackError],
            "database transaction and rollback failed",
          );
        }
        throw error;
      }
    };
  }
}

let database: SettingsDatabase | null = null;

function isLegacyEncryptedString(value: string): boolean {
  return value.startsWith(LEGACY_ENCRYPTED_VALUE_PREFIX);
}

function decryptLegacyString(value: string): string | null {
  if (!isLegacyEncryptedString(value)) {
    return value;
  }
  if (!safeStorage.isEncryptionAvailable()) {
    return null;
  }
  try {
    return safeStorage.decryptString(
      Buffer.from(value.slice(LEGACY_ENCRYPTED_VALUE_PREFIX.length), "base64"),
    );
  } catch {
    return null;
  }
}

function migrateLegacyEncryptedStrings(store: SettingsDatabase): void {
  const profileRows = store
    .prepare("SELECT id, remote_url FROM profiles WHERE remote_url IS NOT NULL")
    .all() as unknown as { id: string; remote_url: string }[];
  const serverRows = store
    .prepare("SELECT id, url, secret FROM remote_servers")
    .all() as unknown as { id: string; url: string; secret: string }[];
  const githubTokenRow = store
    .prepare("SELECT data FROM preferences WHERE name = 'github_token'")
    .get() as Pick<PreferenceRow, "data"> | undefined;
  const githubTokenValue =
    githubTokenRow === undefined ? undefined : decodePreference(githubTokenRow.data);
  const encryptedGithubToken =
    typeof githubTokenValue === "string" &&
    isLegacyEncryptedString(githubTokenValue)
      ? githubTokenValue
      : undefined;
  const encryptedProfiles = profileRows.filter((row) =>
    isLegacyEncryptedString(row.remote_url),
  );
  const encryptedServers = serverRows.filter(
    (row) =>
      isLegacyEncryptedString(row.url) || isLegacyEncryptedString(row.secret),
  );
  if (
    encryptedGithubToken === undefined &&
    encryptedProfiles.length === 0 &&
    encryptedServers.length === 0
  ) {
    return;
  }

  store.transaction(() => {
    const updateProfile = store.prepare("UPDATE profiles SET remote_url = ? WHERE id = ?");
    for (const row of encryptedProfiles) {
      updateProfile.run(decryptLegacyString(row.remote_url), row.id);
    }
    const updateServer = store.prepare(
      "UPDATE remote_servers SET url = ?, secret = ? WHERE id = ?",
    );
    const deleteServer = store.prepare("DELETE FROM remote_servers WHERE id = ?");
    const deletedServerIds = new Set<string>();
    for (const row of encryptedServers) {
      const url = decryptLegacyString(row.url);
      const secret = decryptLegacyString(row.secret);
      if (url === null || secret === null) {
        deleteServer.run(row.id);
        deletedServerIds.add(row.id);
      } else {
        updateServer.run(url, secret, row.id);
      }
    }
    if (deletedServerIds.size > 0) {
      const activeServerRow = store
        .prepare("SELECT data FROM preferences WHERE name = 'active_remote_server_id'")
        .get() as Pick<PreferenceRow, "data"> | undefined;
      const activeServerId =
        activeServerRow === undefined ? undefined : decodePreference(activeServerRow.data);
      if (typeof activeServerId === "string" && deletedServerIds.has(activeServerId)) {
        store
          .prepare("DELETE FROM preferences WHERE name = 'active_remote_server_id'")
          .run();
      }
    }
    if (encryptedGithubToken !== undefined) {
      const githubToken = decryptLegacyString(encryptedGithubToken);
      if (githubToken === null) {
        store.prepare("DELETE FROM preferences WHERE name = 'github_token'").run();
      } else {
        store
          .prepare("UPDATE preferences SET data = ? WHERE name = 'github_token'")
          .run(encodePreference(githubToken));
      }
    }
  })();
}

function createSchema(store: SettingsDatabase): void {
  store.exec(
    `CREATE TABLE IF NOT EXISTS profiles (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      type TEXT NOT NULL,
      remote_url TEXT,
      auto_update INTEGER NOT NULL,
      auto_update_interval_minutes INTEGER NOT NULL,
      last_updated INTEGER,
      item_order INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS preferences (
      name TEXT PRIMARY KEY,
      data BLOB NOT NULL
    );
    CREATE TABLE IF NOT EXISTS remote_servers (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      url TEXT NOT NULL,
      secret TEXT NOT NULL,
      item_order INTEGER NOT NULL
    );`,
  );
}

export function settingsDatabase(): SettingsDatabase {
  if (database !== null) {
    return database;
  }
  mkdirSync(app.getPath("userData"), { recursive: true });
  const store = new SettingsDatabase(
    join(app.getPath("userData"), "settings.db"),
  );
  try {
    store.exec("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;");
    createSchema(store);
    migrateLegacyEncryptedStrings(store);
  } catch (error) {
    store.close();
    throw error;
  }
  database = store;
  return store;
}

function encodePreference(value: unknown): Buffer {
  const encoded = JSON.stringify(value);
  if (encoded === undefined) {
    throw new Error("preference value is not serializable");
  }
  const data = Buffer.from(encoded);
  if (data.byteLength > MAXIMUM_PREFERENCE_BYTES) {
    throw new Error("preference value is too large");
  }
  return data;
}

function decodePreference(data: Uint8Array): unknown {
  return JSON.parse(Buffer.from(data).toString("utf-8")) as unknown;
}

export function parseBooleanPreference(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid boolean preference");
  }
  return value;
}

export class Preference<Value> {
  constructor(
    readonly name: string,
    private readonly defaultValue: Value,
    private readonly parse: (value: unknown) => Value,
  ) {}

  get(): Value {
    const row = settingsDatabase()
      .prepare("SELECT data FROM preferences WHERE name = ?")
      .get(this.name) as Pick<PreferenceRow, "data"> | undefined;
    if (row === undefined) {
      return this.defaultValue;
    }
    return this.parse(decodePreference(row.data));
  }

  set(value: Value | null): void {
    if (value === null) {
      settingsDatabase().prepare("DELETE FROM preferences WHERE name = ?").run(this.name);
      return;
    }
    settingsDatabase()
      .prepare(
        `INSERT INTO preferences (name, data) VALUES (?, ?)
         ON CONFLICT(name) DO UPDATE SET data = excluded.data`,
      )
      .run(this.name, encodePreference(value));
  }
}

export function preferenceSnapshot(names: readonly string[]): Record<string, unknown> {
  if (names.length === 0) {
    return {};
  }
  const placeholders = names.map(() => "?").join(", ");
  const rows = settingsDatabase()
    .prepare(`SELECT name, data FROM preferences WHERE name IN (${placeholders})`)
    .all(...names) as unknown as PreferenceRow[];
  return Object.fromEntries(rows.map((row) => [row.name, decodePreference(row.data)]));
}

export function setPreference(name: string, value: unknown): void {
  if (name === "" || name.length > 128) {
    throw new Error("invalid preference name");
  }
  settingsDatabase()
    .prepare(
      `INSERT INTO preferences (name, data) VALUES (?, ?)
       ON CONFLICT(name) DO UPDATE SET data = excluded.data`,
    )
    .run(name, encodePreference(value));
}

export function removePreference(name: string): void {
  settingsDatabase().prepare("DELETE FROM preferences WHERE name = ?").run(name);
}
