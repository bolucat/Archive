import { BrowserWindow, ipcMain, session, shell } from "electron";
import type { Cookie, Event, WebContents } from "electron";

import {
  OPENCONNECT_BROWSER_AUTHENTICATE,
  OPENCONNECT_BROWSER_CANCEL,
} from "../shared/ipc";
import type {
  OpenConnectBrowserRequest,
  OpenConnectBrowserResult,
} from "../shared/ipc";

let activeAuthentication: {
  browserSessionID: string;
  ownerWebContentsID: number;
  cancel: () => void;
} | null = null;

type BrowserAuthenticationState =
  | "opening"
  | "active"
  | "completing"
  | "canceled";

type CookieCandidate = {
  cookie: Cookie;
  sequence: number;
};

type CookieCompletion = {
  url: string;
  generation: number;
  timer: ReturnType<typeof setTimeout> | null;
};

type CookieCompletionToken = {
  webContentsID: number;
  generation: number;
};

const MAXIMUM_BROWSER_WINDOW_COUNT = 9;

function cookieIdentity(cookie: Cookie): string {
  return [
    cookie.name,
    cookie.domain?.toLowerCase() ?? "",
    cookie.path ?? "/",
    cookie.hostOnly ? "host" : "domain",
  ].join("\0");
}

function isLiveCookie(cookie: Cookie): boolean {
  return (
    cookie.value !== "" &&
    (cookie.expirationDate === undefined ||
      cookie.expirationDate > Date.now() / 1000)
  );
}

function cookieMatchesURL(cookie: Cookie, value: string): boolean {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return false;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return false;
  const hostname = url.hostname.toLowerCase();
  const domain = (cookie.domain ?? "").toLowerCase().replace(/^\./, "");
  if (
    domain === "" ||
    (cookie.hostOnly
      ? hostname !== domain
      : hostname !== domain && !hostname.endsWith(`.${domain}`))
  )
    return false;
  const requestPath = url.pathname === "" ? "/" : url.pathname;
  const cookiePath =
    cookie.path === undefined || cookie.path === "" ? "/" : cookie.path;
  if (
    requestPath !== cookiePath &&
    (!requestPath.startsWith(cookiePath) ||
      (!cookiePath.endsWith("/") && requestPath[cookiePath.length] !== "/"))
  )
    return false;
  return !cookie.secure || url.protocol === "https:";
}

function parseRequest(value: unknown): OpenConnectBrowserRequest {
  if (typeof value !== "object" || value === null) {
    throw new Error("invalid OpenConnect browser request");
  }
  const request = value as Partial<OpenConnectBrowserRequest>;
  if (
    typeof request.url !== "string" ||
    typeof request.finalURL !== "string" ||
    !Array.isArray(request.cookieNames) ||
    !request.cookieNames.every((name) => typeof name === "string") ||
    !Array.isArray(request.earlyCookieNames) ||
    !request.earlyCookieNames.every((name) => typeof name === "string") ||
    !Array.isArray(request.headerNames) ||
    !request.headerNames.every((name) => typeof name === "string") ||
    !Array.isArray(request.callbackURLPrefixes) ||
    !request.callbackURLPrefixes.every(
      (prefix) => typeof prefix === "string" && prefix.length > 0,
    )
  ) {
    throw new Error("invalid OpenConnect browser request");
  }
  const scheme = new URL(request.url).protocol;
  if (scheme !== "http:" && scheme !== "https:" && scheme !== "data:") {
    throw new Error(`unsupported OpenConnect browser URL scheme: ${scheme}`);
  }
  const callbackMode =
    request.callbackURLPrefixes.length > 0 &&
    request.headerNames.length === 0 &&
    request.cookieNames.length === 0 &&
    request.earlyCookieNames.length === 0 &&
    request.finalURL === "";
  const headerMode =
    request.callbackURLPrefixes.length === 0 &&
    request.headerNames.length > 0 &&
    request.cookieNames.length === 0 &&
    request.earlyCookieNames.length === 0 &&
    request.finalURL === "";
  const cookieMode =
    request.callbackURLPrefixes.length === 0 &&
    request.headerNames.length === 0 &&
    request.cookieNames.length > 0 &&
    request.finalURL !== "";
  const normalCookieNames = new Set(request.cookieNames);
  const earlyCookieNames = new Set(request.earlyCookieNames);
  const callbackURLPrefixes = new Set(request.callbackURLPrefixes);
  const headerNames = new Set(
    request.headerNames.map((name) => name.toLowerCase()),
  );
  const invalidCookieNames =
    normalCookieNames.size !== request.cookieNames.length ||
    earlyCookieNames.size !== request.earlyCookieNames.length ||
    request.cookieNames.some(
      (name) => name === "" || earlyCookieNames.has(name),
    ) ||
    request.earlyCookieNames.some((name) => name === "");
  const invalidCallbackURLPrefixes =
    callbackURLPrefixes.size !== request.callbackURLPrefixes.length;
  const invalidHeaderNames =
    headerNames.size !== request.headerNames.length ||
    request.headerNames.some((name) => name === "");
  if (
    (!callbackMode && !headerMode && !cookieMode) ||
    invalidCookieNames ||
    invalidCallbackURLPrefixes ||
    invalidHeaderNames
  ) {
    throw new Error("invalid OpenConnect browser completion mode");
  }
  return {
    url: request.url,
    finalURL: request.finalURL,
    cookieNames: [...request.cookieNames],
    earlyCookieNames: [...request.earlyCookieNames],
    headerNames: [...request.headerNames],
    callbackURLPrefixes: [...request.callbackURLPrefixes],
  };
}

function requestedHeaders(
  request: OpenConnectBrowserRequest,
  responseHeaders: Record<string, string[]> | undefined,
): OpenConnectBrowserResult["headers"] {
  const requestedNames = new Set(
    request.headerNames.map((name) => name.toLowerCase()),
  );
  const headers: OpenConnectBrowserResult["headers"] = [];
  for (const [name, values] of Object.entries(responseHeaders ?? {})) {
    if (requestedNames.has(name.toLowerCase())) {
      headers.push({ name, values: [...values] });
    }
  }
  return headers;
}

function isAllowedNavigationURL(value: string): boolean {
  try {
    const scheme = new URL(value).protocol;
    return (
      scheme === "http:" ||
      scheme === "https:" ||
      scheme === "data:" ||
      scheme === "about:" ||
      scheme === "blob:"
    );
  } catch {
    return false;
  }
}

function headersComplete(
  request: OpenConnectBrowserRequest,
  headers: OpenConnectBrowserResult["headers"],
): boolean {
  const names = new Set(
    headers
      .filter((header) => header.values.some((value) => value.trim() !== ""))
      .map((header) => header.name.toLowerCase()),
  );
  const requestedNames = new Set(
    request.headerNames.map((name) => name.toLowerCase()),
  );
  if (
    requestedNames.has("saml-username") &&
    requestedNames.has("prelogin-cookie") &&
    requestedNames.has("portal-userauthcookie")
  ) {
    return (
      names.has("saml-username") &&
      (names.has("prelogin-cookie") || names.has("portal-userauthcookie"))
    );
  }
  return [...requestedNames].every((name) => names.has(name));
}

async function authenticate(
  parent: BrowserWindow,
  browserSessionID: string,
  storageID: string,
  request: OpenConnectBrowserRequest,
): Promise<OpenConnectBrowserResult | null> {
  if (activeAuthentication !== null) {
    throw new Error(
      "another OpenConnect browser authentication is already active",
    );
  }
  const browserPartition = `persist:openconnect-browser-${storageID}`;
  const browserSession = session.fromPartition(browserPartition);
  const windows = new Set<BrowserWindow>();
  const collectedCookies = new Map<string, { name: string; value: string }>();
  const cookieCandidates = new Map<string, CookieCandidate>();
  const cookieMutationSequences = new Map<string, number>();
  const collectedHeaders = new Map<
    string,
    { name: string; values: string[] }
  >();
  const cookieCompletions = new Map<number, CookieCompletion>();
  let cookieSequence = 0;
  let cookieCompletionGeneration = 0;
  let state: BrowserAuthenticationState = "opening";

  return new Promise<OpenConnectBrowserResult | null>((resolve, reject) => {
    const finished = () => state === "completing" || state === "canceled";
    const cookieListener = (
      _event: Event,
      cookie: Cookie,
      _cause: string,
      removed: boolean,
    ) => {
      if (finished()) return;
      const requested =
        request.cookieNames.includes(cookie.name) ||
        request.earlyCookieNames.includes(cookie.name);
      if (!requested) return;
      const identity = cookieIdentity(cookie);
      cookieSequence++;
      cookieMutationSequences.set(identity, cookieSequence);
      if (removed || !isLiveCookie(cookie)) {
        cookieCandidates.delete(identity);
      } else {
        cookieCandidates.set(identity, { cookie, sequence: cookieSequence });
      }
      if (
        !removed &&
        isLiveCookie(cookie) &&
        request.earlyCookieNames.includes(cookie.name)
      ) {
        finish({
          finalURL: "",
          cookies: [{ name: cookie.name, value: cookie.value }],
          headers: [],
        });
        return;
      }
      if (!request.cookieNames.includes(cookie.name)) return;
      scheduleCookieCompletion();
    };
    const cleanup = () => {
      for (const completion of cookieCompletions.values()) {
        if (completion.timer !== null) clearTimeout(completion.timer);
      }
      cookieCompletions.clear();
      browserSession.cookies.off("changed", cookieListener);
      browserSession.webRequest.onHeadersReceived(null);
      if (activeAuthentication?.cancel === cancel) activeAuthentication = null;
    };
    const finish = (result: OpenConnectBrowserResult | null) => {
      if (finished()) return;
      state = result === null ? "canceled" : "completing";
      cleanup();
      resolve(result);
      for (const window of windows) {
        if (!window.isDestroyed()) window.close();
      }
    };
    const fail = (error: unknown) => {
      if (finished()) return;
      state = "canceled";
      cleanup();
      reject(error);
      for (const window of windows) {
        if (!window.isDestroyed()) window.close();
      }
    };
    const cancel = () => finish(null);
    const validCookieCompletion = (token: CookieCompletionToken) =>
      cookieCompletions.get(token.webContentsID)?.generation ===
      token.generation;
    const maybeFinishCookies = (token: CookieCompletionToken) => {
      if (
        finished() ||
        !validCookieCompletion(token) ||
        collectedCookies.size === 0
      )
        return;
      const completion = cookieCompletions.get(token.webContentsID);
      if (!completion || completion.url !== request.finalURL) return;
      finish({
        finalURL: request.finalURL,
        cookies: [...collectedCookies.values()],
        headers: [],
      });
    };
    const scheduleCookieCompletion = () => {
      for (const [webContentsID, completion] of cookieCompletions) {
        if (completion.timer !== null) continue;
        const token = { webContentsID, generation: completion.generation };
        completion.timer = setTimeout(() => {
          const currentCompletion = cookieCompletions.get(webContentsID);
          if (currentCompletion?.generation !== token.generation) return;
          currentCompletion.timer = null;
          void captureCookies(currentCompletion.url, true, token).catch(fail);
        }, 100);
      }
    };
    const clearCookieCompletion = (contents: WebContents) => {
      const completion = cookieCompletions.get(contents.id);
      if (!completion) return;
      if (completion.timer !== null) clearTimeout(completion.timer);
      cookieCompletions.delete(contents.id);
    };
    const setCookieCompletion = (
      contents: WebContents,
      url: string,
    ): CookieCompletionToken => {
      clearCookieCompletion(contents);
      cookieCompletionGeneration++;
      cookieCompletions.set(contents.id, {
        url,
        generation: cookieCompletionGeneration,
        timer: null,
      });
      return {
        webContentsID: contents.id,
        generation: cookieCompletionGeneration,
      };
    };
    const synchronizeCookieCandidates = (
      cookies: Cookie[],
      rebuild: boolean,
      snapshotSequence: number,
    ) => {
      const requestedNames = new Set([
        ...request.cookieNames,
        ...request.earlyCookieNames,
      ]);
      const liveCookies = cookies
        .filter(
          (cookie) => requestedNames.has(cookie.name) && isLiveCookie(cookie),
        )
        .sort((left, right) =>
          cookieIdentity(left).localeCompare(cookieIdentity(right)),
        );
      const liveIdentities = new Set(liveCookies.map(cookieIdentity));
      if (rebuild) {
        for (const identity of cookieCandidates.keys()) {
          if (
            !liveIdentities.has(identity) &&
            (cookieMutationSequences.get(identity) ?? 0) <= snapshotSequence
          ) {
            cookieSequence++;
            cookieMutationSequences.set(identity, cookieSequence);
            cookieCandidates.delete(identity);
          }
        }
      }
      for (const cookie of liveCookies) {
        const identity = cookieIdentity(cookie);
        if ((cookieMutationSequences.get(identity) ?? 0) > snapshotSequence)
          continue;
        const existing = cookieCandidates.get(identity);
        if (
          existing?.cookie.value === cookie.value &&
          existing.cookie.secure === cookie.secure &&
          existing.cookie.expirationDate === cookie.expirationDate
        )
          continue;
        cookieSequence++;
        cookieMutationSequences.set(identity, cookieSequence);
        cookieCandidates.set(identity, { cookie, sequence: cookieSequence });
      }
    };
    const selectCookie = (name: string, preferredURL: string) =>
      [...cookieCandidates.entries()]
        .filter(
          ([, candidate]) =>
            candidate.cookie.name === name && isLiveCookie(candidate.cookie),
        )
        .sort(([leftIdentity, left], [rightIdentity, right]) => {
          const leftMatches = cookieMatchesURL(left.cookie, preferredURL);
          const rightMatches = cookieMatchesURL(right.cookie, preferredURL);
          if (leftMatches !== rightMatches) return leftMatches ? -1 : 1;
          if (left.sequence !== right.sequence)
            return right.sequence - left.sequence;
          return leftIdentity.localeCompare(rightIdentity);
        })[0]?.[1].cookie;
    const rebuildCollectedCookies = (preferredURL: string) => {
      collectedCookies.clear();
      for (const name of request.cookieNames) {
        const cookie = selectCookie(name, preferredURL);
        if (cookie) collectedCookies.set(name, { name, value: cookie.value });
      }
    };
    const captureCookies = async (
      url: string,
      rebuild: boolean = false,
      completionToken?: CookieCompletionToken,
    ) => {
      if (
        finished() ||
        request.cookieNames.length === 0 ||
        !/^https?:/i.test(url)
      )
        return;
      const snapshotSequence = cookieSequence;
      const cookies = await browserSession.cookies.get(rebuild ? {} : { url });
      if (finished()) return;
      synchronizeCookieCandidates(cookies, rebuild, snapshotSequence);
      for (const name of request.earlyCookieNames) {
        const cookie = selectCookie(name, "");
        if (cookie) {
          finish({
            finalURL: "",
            cookies: [{ name: cookie.name, value: cookie.value }],
            headers: [],
          });
          return;
        }
      }
      if (completionToken && !validCookieCompletion(completionToken)) return;
      rebuildCollectedCookies(url);
      if (completionToken) maybeFinishCookies(completionToken);
    };
    const isCallbackURL = (url: string) =>
      request.callbackURLPrefixes.some((prefix) => url.startsWith(prefix));
    const completeCallback = (url: string) => {
      if (!isCallbackURL(url)) return false;
      finish({ finalURL: url, cookies: [], headers: [] });
      return true;
    };
    const attachContents = (
      contents: WebContents,
      rootWindow: BrowserWindow,
    ) => {
      contents.setWindowOpenHandler(({ url }) => {
        if (completeCallback(url)) return { action: "deny" };
        let scheme: string;
        try {
          scheme = new URL(url).protocol;
        } catch {
          fail(
            new Error(
              `OpenConnect browser blocked an invalid popup URL: ${url}`,
            ),
          );
          return { action: "deny" };
        }
        if (isAllowedNavigationURL(url)) {
          if (windows.size >= MAXIMUM_BROWSER_WINDOW_COUNT) {
            fail(
              new Error(
                "OpenConnect authentication opened too many browser windows",
              ),
            );
            return { action: "deny" };
          }
          return {
            action: "allow",
            overrideBrowserWindowOptions: {
              parent: rootWindow,
              autoHideMenuBar: true,
              webPreferences: {
                partition: browserPartition,
                contextIsolation: true,
                sandbox: true,
                nodeIntegration: false,
              },
            },
          };
        }
        if (scheme === "mailto:" || scheme === "tel:") {
          void shell.openExternal(url).catch(() => {});
        } else {
          fail(
            new Error(`OpenConnect browser blocked popup navigation to ${url}`),
          );
        }
        return { action: "deny" };
      });
      contents.on("will-navigate", (event) => {
        if (event.isMainFrame) clearCookieCompletion(contents);
        if (event.isMainFrame && isCallbackURL(event.url)) {
          event.preventDefault();
          finish({ finalURL: event.url, cookies: [], headers: [] });
        } else if (event.isMainFrame && !isAllowedNavigationURL(event.url)) {
          event.preventDefault();
          fail(
            new Error(`OpenConnect browser blocked navigation to ${event.url}`),
          );
        }
      });
      contents.on("will-redirect", (event) => {
        if (event.isMainFrame) clearCookieCompletion(contents);
        if (event.isMainFrame && isCallbackURL(event.url)) {
          event.preventDefault();
          finish({ finalURL: event.url, cookies: [], headers: [] });
        } else if (event.isMainFrame && !isAllowedNavigationURL(event.url)) {
          event.preventDefault();
          fail(
            new Error(`OpenConnect browser blocked redirect to ${event.url}`),
          );
        }
      });
      contents.on("did-create-window", (window) => {
        windows.add(window);
        attachContents(window.webContents, rootWindow);
        window.on("closed", () => windows.delete(window));
      });
      contents.on("did-finish-load", () => {
        const url = contents.getURL();
        if (url === request.finalURL) {
          const token = setCookieCompletion(contents, url);
          void captureCookies(url, true, token).catch(fail);
        } else {
          clearCookieCompletion(contents);
          void captureCookies(url).catch(fail);
        }
      });
      contents.on("destroyed", () => clearCookieCompletion(contents));
      contents.on("render-process-gone", (_event, details) => {
        fail(
          new Error(`OpenConnect browser process stopped: ${details.reason}`),
        );
      });
      contents.on(
        "did-fail-load",
        (_event, errorCode, errorDescription, validatedURL, isMainFrame) => {
          if (isMainFrame && errorCode !== -3) {
            fail(
              new Error(
                `OpenConnect browser navigation failed for ${validatedURL}: ${errorDescription}`,
              ),
            );
          }
        },
      );
    };

    activeAuthentication = {
      browserSessionID,
      ownerWebContentsID: parent.webContents.id,
      cancel,
    };
    void (async () => {
      state = "active";
      const requestedCookieNames = new Set([
        ...request.cookieNames,
        ...request.earlyCookieNames,
      ]);
      if (requestedCookieNames.size > 0) {
        const staleCookies = (await browserSession.cookies.get({})).filter(
          (cookie) => requestedCookieNames.has(cookie.name),
        );
        for (const cookie of staleCookies) {
          const domain = (cookie.domain ?? "").replace(/^\./, "");
          if (domain === "") continue;
          const cookiePath =
            cookie.path === undefined || cookie.path === "" ? "/" : cookie.path;
          const cookieURL = `${cookie.secure ? "https" : "http"}://${domain}${cookiePath}`;
          await browserSession.cookies.remove(cookieURL, cookie.name);
        }
      }
      browserSession.cookies.on("changed", cookieListener);
      browserSession.webRequest.onHeadersReceived(
        { urls: ["*://*/*"] },
        (details, callback) => {
          if (details.resourceType === "mainFrame") {
            const headers = requestedHeaders(request, details.responseHeaders);
            if (details.statusCode >= 400) {
              fail(
                new Error(
                  `OpenConnect browser navigation failed for ${details.url}: HTTP ${details.statusCode}`,
                ),
              );
            } else if (request.headerNames.length > 0) {
              for (const header of headers) {
                if (header.values.some((value) => value.trim() !== "")) {
                  collectedHeaders.set(header.name.toLowerCase(), header);
                }
              }
              const accumulatedHeaders = request.headerNames.flatMap((name) => {
                const header = collectedHeaders.get(name.toLowerCase());
                return header ? [header] : [];
              });
              if (headersComplete(request, accumulatedHeaders)) {
                finish({
                  finalURL: "",
                  cookies: [],
                  headers: accumulatedHeaders,
                });
              }
            }
          }
          callback({ responseHeaders: details.responseHeaders });
        },
      );

      const rootWindow = new BrowserWindow({
        parent,
        width: 980,
        height: 760,
        minWidth: 640,
        minHeight: 480,
        show: false,
        title: "OpenConnect Authentication",
        autoHideMenuBar: true,
        webPreferences: {
          partition: browserPartition,
          contextIsolation: true,
          sandbox: true,
          nodeIntegration: false,
        },
      });
      windows.add(rootWindow);
      attachContents(rootWindow.webContents, rootWindow);
      rootWindow.once("ready-to-show", () => rootWindow.show());
      rootWindow.on("closed", () => {
        windows.delete(rootWindow);
        if (!finished()) finish(null);
      });
      await rootWindow.loadURL(request.url);
    })().catch(fail);
  });
}

export function registerOpenConnectBrowser() {
  ipcMain.handle(
    OPENCONNECT_BROWSER_AUTHENTICATE,
    (
      event,
      browserSessionID: unknown,
      storageID: unknown,
      requestValue: unknown,
    ) => {
      const parent = BrowserWindow.fromWebContents(event.sender);
      if (parent === null) {
        throw new Error(
          "OpenConnect browser authentication requires an application window",
        );
      }
      if (typeof browserSessionID !== "string" || browserSessionID === "") {
        throw new Error("invalid OpenConnect browser session ID");
      }
      if (typeof storageID !== "string" || storageID === "") {
        throw new Error("invalid OpenConnect browser storage ID");
      }
      return authenticate(
        parent,
        browserSessionID,
        storageID,
        parseRequest(requestValue),
      );
    },
  );
  ipcMain.on(OPENCONNECT_BROWSER_CANCEL, (event, browserSessionID: unknown) => {
    if (
      typeof browserSessionID === "string" &&
      activeAuthentication?.browserSessionID === browserSessionID &&
      activeAuthentication.ownerWebContentsID === event.sender.id
    ) {
      activeAuthentication.cancel();
    }
  });
}
