import { app } from "electron";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

interface PreferenceRow {
  name: string;
  data: Uint8Array;
}

const MAXIMUM_PREFERENCE_BYTES = 1024 * 1024;

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
