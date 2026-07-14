import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  loadServersState,
  normalizeServerUrl,
  removeServer,
  saveServersState,
  serverConnectUrl,
  serverDisplayName,
  upsertServer,
  type Server,
  type ServersState,
} from "./config";

class MemoryStorage {
  private map = new Map<string, string>();

  getItem(key: string): string | null {
    return this.map.get(key) ?? null;
  }

  setItem(key: string, value: string) {
    this.map.set(key, String(value));
  }

  removeItem(key: string) {
    this.map.delete(key);
  }
}

const STORAGE_KEY = "sing-box-dashboard.servers";
const LEGACY_STORAGE_KEY = "sing-box-dashboard.server";

beforeEach(() => {
  vi.stubGlobal("localStorage", new MemoryStorage());
});

describe("loadServersState", () => {
  it("returns an empty state when nothing is stored", () => {
    expect(loadServersState()).toEqual({ servers: [], activeId: null });
  });

  it("round-trips through saveServersState and drops the http:// scheme", () => {
    saveServersState({
      servers: [{ id: "a", name: "Home", url: "http://10.0.0.1:9090", secret: "s" }],
      activeId: "a",
    });
    expect(loadServersState()).toEqual({
      servers: [{ id: "a", name: "Home", url: "10.0.0.1:9090", secret: "s" }],
      activeId: "a",
    });
  });

  it("survives malformed JSON", () => {
    localStorage.setItem(STORAGE_KEY, "{not json");
    expect(loadServersState()).toEqual({ servers: [], activeId: null });
  });

  it("drops entries without an id or url and fixes a dangling activeId", () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        servers: [
          { id: "a", url: "http://10.0.0.1:9090" },
          { id: "b", url: "" },
          { url: "http://no-id.example" },
          null,
        ],
        activeId: "gone",
      }),
    );
    const state = loadServersState();
    expect(state.servers.map((server) => server.id)).toEqual(["a"]);
    expect(state.activeId).toBe("a");
    expect(state.servers[0]).toEqual({ id: "a", name: "", secret: "", url: "10.0.0.1:9090" });
  });

  it("migrates the legacy single-server entry and removes it", () => {
    localStorage.setItem(LEGACY_STORAGE_KEY, JSON.stringify({ url: "http://old.example", secret: "s" }));
    const state = loadServersState();
    expect(state.servers).toHaveLength(1);
    expect(state.servers[0].url).toBe("old.example");
    expect(state.servers[0].secret).toBe("s");
    expect(state.activeId).toBe(state.servers[0].id);
    expect(localStorage.getItem(LEGACY_STORAGE_KEY)).toBeNull();
    expect(localStorage.getItem(STORAGE_KEY)).not.toBeNull();
  });
});

describe("normalizeServerUrl", () => {
  it("drops the implied http:// scheme and trailing slashes", () => {
    expect(normalizeServerUrl(" http://10.0.0.1:9090/ ")).toBe("10.0.0.1:9090");
    expect(normalizeServerUrl("10.0.0.1:9090/")).toBe("10.0.0.1:9090");
    expect(normalizeServerUrl("https://example.com//")).toBe("https://example.com");
    expect(normalizeServerUrl("")).toBe("");
  });
});

describe("serverConnectUrl", () => {
  it("re-adds the implied http:// scheme and strips trailing slashes", () => {
    expect(serverConnectUrl("10.0.0.1:9090")).toBe("http://10.0.0.1:9090");
    expect(serverConnectUrl(" 10.0.0.1:9090/ ")).toBe("http://10.0.0.1:9090");
    expect(serverConnectUrl("http://x")).toBe("http://x");
    expect(serverConnectUrl("https://example.com/")).toBe("https://example.com");
    expect(serverConnectUrl("")).toBe("");
  });
});

describe("upsertServer / removeServer", () => {
  const server = (id: string): Server => ({ id, name: "", url: `http://${id}.example`, secret: "" });

  it("appends a new server and activates it when nothing is active", () => {
    const state: ServersState = { servers: [], activeId: null };
    const next = upsertServer(state, server("a"));
    expect(next.servers.map((entry) => entry.id)).toEqual(["a"]);
    expect(next.activeId).toBe("a");
  });

  it("appends without stealing the active server", () => {
    const state: ServersState = { servers: [server("a")], activeId: "a" };
    const next = upsertServer(state, server("b"));
    expect(next.servers.map((entry) => entry.id)).toEqual(["a", "b"]);
    expect(next.activeId).toBe("a");
  });

  it("replaces an existing server in place", () => {
    const state: ServersState = { servers: [server("a"), server("b")], activeId: "b" };
    const next = upsertServer(state, { ...server("a"), name: "Renamed" });
    expect(next.servers.map((entry) => entry.name)).toEqual(["Renamed", ""]);
    expect(next.activeId).toBe("b");
  });

  it("removes a server and falls back to the first remaining one", () => {
    const state: ServersState = { servers: [server("a"), server("b")], activeId: "a" };
    expect(removeServer(state, "a")).toEqual({ servers: [server("b")], activeId: "b" });
    expect(removeServer(state, "b").activeId).toBe("a");
    expect(removeServer({ servers: [server("a")], activeId: "a" }, "a")).toEqual({
      servers: [],
      activeId: null,
    });
  });
});

describe("serverDisplayName", () => {
  it("prefers the name, then the URL host, then the raw URL", () => {
    expect(serverDisplayName({ id: "a", name: "Home", url: "x", secret: "" })).toBe("Home");
    expect(serverDisplayName({ id: "a", name: "", url: "h:9090", secret: "" })).toBe("h:9090");
    expect(serverDisplayName({ id: "a", name: "", url: "https://example.com", secret: "" })).toBe(
      "example.com",
    );
    expect(serverDisplayName({ id: "a", name: "", url: "not a url", secret: "" })).toBe("not a url");
  });
});
