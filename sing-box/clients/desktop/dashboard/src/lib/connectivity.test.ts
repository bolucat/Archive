import { describe, expect, it } from "vitest";

import {
  guessApiBaseUrl,
  isLoopbackHost,
  isOpaqueNetworkError,
  isUnknownServiceError,
} from "./connectivity";

describe("isOpaqueNetworkError", () => {
  it("matches the per-engine wordings", () => {
    expect(isOpaqueNetworkError("Failed to fetch")).toBe(true);
    expect(isOpaqueNetworkError("Load failed")).toBe(true);
    expect(isOpaqueNetworkError("NetworkError when attempting to fetch resource.")).toBe(true);
  });

  it("passes daemon errors through", () => {
    expect(isOpaqueNetworkError("bad secret")).toBe(false);
    expect(isOpaqueNetworkError("Stream ended without a status message")).toBe(false);
  });
});

describe("isUnknownServiceError", () => {
  it("matches a gRPC unimplemented for the daemon service", () => {
    expect(isUnknownServiceError("[unimplemented] unknown service daemon.StartedService")).toBe(
      true,
    );
    expect(
      isUnknownServiceError("[unimplemented] unknown service some/prefix/daemon.StartedService"),
    ).toBe(true);
  });

  it("ignores other errors", () => {
    expect(isUnknownServiceError("[unauthenticated] bad secret")).toBe(false);
    expect(isUnknownServiceError("Failed to fetch")).toBe(false);
    expect(isUnknownServiceError("[unimplemented] unknown method GetVersion")).toBe(false);
  });
});

describe("isLoopbackHost", () => {
  it("recognizes loopback hosts", () => {
    expect(isLoopbackHost("localhost")).toBe(true);
    expect(isLoopbackHost("foo.localhost")).toBe(true);
    expect(isLoopbackHost("127.0.0.1")).toBe(true);
    expect(isLoopbackHost("127.1.2.3")).toBe(true);
    expect(isLoopbackHost("[::1]")).toBe(true);
  });

  it("rejects everything else", () => {
    expect(isLoopbackHost("192.168.1.2")).toBe(false);
    expect(isLoopbackHost("example.com")).toBe(false);
    expect(isLoopbackHost("127.0.0.1.evil.com")).toBe(false);
  });
});

describe("guessApiBaseUrl", () => {
  it("strips the dashboard subdirectory to the parent path", () => {
    expect(guessApiBaseUrl("https://host/dashboard/")).toBe("https://host/");
    expect(guessApiBaseUrl("https://host/dashboard/index.html")).toBe("https://host/");
    expect(guessApiBaseUrl("http://192.168.1.1:9090/ui/?x=1#/settings")).toBe(
      "http://192.168.1.1:9090/",
    );
  });

  it("strips only one level for nested paths", () => {
    expect(guessApiBaseUrl("https://host/admin/dashboard/")).toBe("https://host/admin/");
  });

  it("returns null at the site root", () => {
    expect(guessApiBaseUrl("https://host/")).toBeNull();
    expect(guessApiBaseUrl("https://host/index.html")).toBeNull();
    expect(guessApiBaseUrl("not a url")).toBeNull();
  });
});
