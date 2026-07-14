import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";

import {
  loadServersState,
  saveServersState,
  serverDisplayName,
  type Server,
  type ServersState,
} from "./api/config";
import { DaemonApi } from "./api/daemon";
import { formatDateTime, formatUptime, isHttpUrl } from "./api/format";
import { isTerminalCode, useStream } from "./api/stream";
import { ServiceStatus_Type, type DeprecatedWarning } from "./gen/daemon/started_service_pb";
import { CapabilitiesContext, makeCapabilities } from "./app/capabilities";
import {
  ApiContext,
  applyAccent,
  applyTheme,
  loadAccentPreference,
  loadThemePreference,
  navigate,
  saveAccentPreference,
  saveThemePreference,
  useApi,
  useNow,
  watchSystemTheme,
  type AccentPreference,
  type ThemePreference,
} from "./app/context";
import {
  DesktopHostContext,
  DesktopLocalContext,
  useDaemonConnection,
  useDesktopHost,
  useLocalDesktopHost,
  useRemoteSession,
  type DesktopHost,
} from "./app/desktop";
import { loadDisableDeprecatedWarnings } from "./app/deprecatedWarnings";
import { dismissError, showError, useCurrentError } from "./app/errorStore";
import { useDismiss, useStreamOutage, useUnaryOnce } from "./app/hooks";
import { I18nProvider, useI18n, type Translate } from "./app/i18n";
import {
  DesktopRemoteControls,
  DesktopServerPicker,
  DesktopServiceControls,
  DesktopToolbar,
} from "./components/DesktopToolbar";
import { Icon, type IconName } from "./components/Icon";
import { ToolbarSlotsProvider } from "./components/PageHeader";
import { Brand, Button, Dialog, IconButton, Spinner, StateDot } from "./components/ui";
import { SSH_DEFAULT_TERMINAL_TYPE, SSH_DEFAULT_USERNAME } from "./lib/tailscaleSSH";
import { loadStoredString, saveStoredString } from "./lib/storage";
import { ConnectionErrorView } from "./views/ConnectionErrorView";
import { ConnectionsView } from "./views/ConnectionsView";
import { DesktopSetupView } from "./views/DesktopSetupView";
import {
  CrashReportDetailView,
  CrashReportFileView,
  CrashReportListView,
} from "./views/CrashReportsView";
import { GroupsView } from "./views/GroupsView";
import { LogsView } from "./views/LogsView";
import {
  OOMReportDetailView,
  OOMReportFileView,
  OOMReportListView,
} from "./views/OOMReportsView";
import {
  crashReportFileDisplayName,
  crashReportTitle,
  oomReportFileDisplayName,
  oomReportTitle,
} from "./views/reportFormat";
import { OverviewView } from "./views/OverviewView";
import { ImportProfileFileDialog, ImportRemoteProfileDialog } from "./views/ProfileViews";
import {
  AppSettingsView,
  CoreView,
  PreferencesView,
  ServersView,
  SettingsView,
  TerminalConfigurationView,
  TerminalThemeEditorView,
  TerminalThemePickerView,
} from "./views/SettingsView";
import { SetupView } from "./views/SetupView";
import { NetworkQualityView, STUNTestView, ToolsView } from "./views/ToolsView";
import { TailscaleEndpointView } from "./views/TailscaleView";
import { TailscaleSSHView } from "./views/TerminalView";
import { UsbipView } from "./views/UsbipView";
import styles from "./App.module.css";
import { cx } from "./lib/cx";

export type Route =
  | { page: "overview" }
  | { page: "groups" }
  | { page: "connections" }
  | { page: "logs" }
  | { page: "tools" }
  | { page: "tools/network-quality" }
  | { page: "tools/stun" }
  | { page: "tools/tailscale"; tag: string }
  | { page: "tools/tailscale/ssh"; tag: string; peerID: string; username: string; terminalType: string }
  | { page: "tools/usbip"; tag: string }
  | { page: "tools/crash-reports" }
  | { page: "tools/crash-reports/detail"; name: string; crashedAt: number | null }
  | { page: "tools/crash-reports/file"; name: string; file: string; crashedAt: number | null }
  | { page: "tools/oom-reports" }
  | { page: "tools/oom-reports/detail"; name: string; recordedAt: number | null }
  | { page: "tools/oom-reports/file"; name: string; file: string; recordedAt: number | null }
  | { page: "settings" }
  | { page: "settings/app" }
  | { page: "settings/core" }
  | { page: "settings/preferences" }
  | { page: "settings/preferences/terminal" }
  | { page: "settings/preferences/terminal/theme"; scheme: "light" | "dark" }
  | { page: "settings/preferences/terminal/custom"; scheme: "light" | "dark" }
  | { page: "settings/servers" };

function routeFromHash(locationHash: string): Route {
  const hash = locationHash.replace(/^#\/?/, "");
  const queryIndex = hash.indexOf("?");
  const query = new URLSearchParams(queryIndex >= 0 ? hash.slice(queryIndex + 1) : "");
  const segments = (queryIndex >= 0 ? hash.slice(0, queryIndex) : hash)
    .split("/")
    .map((segment) => {
      try {
        return decodeURIComponent(segment);
      } catch {
        return segment;
      }
    });
  switch (segments[0]) {
    case "groups":
      return { page: "groups" };
    case "connections":
      return { page: "connections" };
    case "logs":
      return { page: "logs" };
    case "tools":
      switch (segments[1]) {
        case "network-quality":
          return { page: "tools/network-quality" };
        case "stun":
          return { page: "tools/stun" };
        case "tailscale":
          if (segments[3] === "ssh" && segments[4]) {
            return {
              page: "tools/tailscale/ssh",
              tag: segments[2] ?? "",
              peerID: segments[4],
              username: query.get("username") || SSH_DEFAULT_USERNAME,
              terminalType: query.get("terminalType") || SSH_DEFAULT_TERMINAL_TYPE,
            };
          }
          return { page: "tools/tailscale", tag: segments[2] ?? "" };
        case "usbip":
          return { page: "tools/usbip", tag: segments[2] ?? "" };
        case "crash-reports": {
          if (segments[2]) {
            const atParam = query.get("at");
            const crashedAt = atParam !== null && /^\d+$/.test(atParam) ? Number(atParam) : null;
            if (segments[3]) {
              return {
                page: "tools/crash-reports/file",
                name: segments[2],
                file: segments[3],
                crashedAt,
              };
            }
            return { page: "tools/crash-reports/detail", name: segments[2], crashedAt };
          }
          return { page: "tools/crash-reports" };
        }
        case "oom-reports": {
          if (segments[2]) {
            const atParam = query.get("at");
            const recordedAt = atParam !== null && /^\d+$/.test(atParam) ? Number(atParam) : null;
            if (segments[3]) {
              return {
                page: "tools/oom-reports/file",
                name: segments[2],
                file: segments[3],
                recordedAt,
              };
            }
            return { page: "tools/oom-reports/detail", name: segments[2], recordedAt };
          }
          return { page: "tools/oom-reports" };
        }
        default:
          return { page: "tools" };
      }
    case "settings":
      switch (segments[1]) {
        case "app":
          return { page: "settings/app" };
        case "core":
          return { page: "settings/core" };
        case "preferences":
          if (segments[2] === "terminal") {
            if (segments[3] === "theme" && (segments[4] === "light" || segments[4] === "dark")) {
              return { page: "settings/preferences/terminal/theme", scheme: segments[4] };
            }
            if (segments[3] === "custom" && (segments[4] === "light" || segments[4] === "dark")) {
              return { page: "settings/preferences/terminal/custom", scheme: segments[4] };
            }
            return { page: "settings/preferences/terminal" };
          }
          return { page: "settings/preferences" };
        case "servers":
          return { page: "settings/servers" };
        default:
          return { page: "settings" };
      }
    default:
      return { page: "overview" };
  }
}

function subscribeLocationHash(onChange: () => void): () => void {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
}

function locationHashSnapshot(): string {
  return location.hash;
}

function isStartedOnlyToolsSubpage(page: string): boolean {
  return page.startsWith("tools/tailscale") || page.startsWith("tools/usbip");
}

function isLocalReportsPage(page: string): boolean {
  return page.startsWith("tools/crash-reports") || page.startsWith("tools/oom-reports");
}

function routeTitle(route: Route, t: Translate, language: string): string {
  switch (route.page) {
    case "overview":
      return t("Dashboard");
    case "groups":
      return t("Groups");
    case "connections":
      return t("Connections");
    case "logs":
      return t("Logs");
    case "tools":
      return t("Tools");
    case "tools/network-quality":
      return t("Network Quality");
    case "tools/stun":
      return t("STUN Test");
    case "tools/tailscale":
      return route.tag !== "" ? t("Tailscale: {tag}", { tag: route.tag }) : "Tailscale";
    case "tools/tailscale/ssh":
      return t("Tools");
    case "tools/usbip":
      return route.tag !== "" ? t("USB/IP: {tag}", { tag: route.tag }) : "USB/IP";
    case "tools/crash-reports":
      return t("Crash Report");
    case "tools/crash-reports/detail":
      return crashReportTitle(route.name, route.crashedAt, language);
    case "tools/crash-reports/file":
      return crashReportFileDisplayName(route.file, t);
    case "tools/oom-reports":
      return t("OOM Report");
    case "tools/oom-reports/detail":
      return oomReportTitle(route.name, route.recordedAt, language);
    case "tools/oom-reports/file":
      return oomReportFileDisplayName(route.file, t);
    case "settings":
      return t("Settings");
    case "settings/app":
      return t("App");
    case "settings/core":
      return t("Core");
    case "settings/preferences":
      return t("Preferences");
    case "settings/preferences/terminal":
      return t("Terminal Configuration");
    case "settings/preferences/terminal/theme":
      return route.scheme === "dark" ? t("Dark") : t("Light");
    case "settings/preferences/terminal/custom":
      return t("Custom theme");
    case "settings/servers":
      return t("Remote Control");
  }
}

const DESKTOP_LOCAL_SERVER: Server = { id: "local", name: "sing-box", url: "", secret: "" };
const DESKTOP_ACTIVE_KEY = "desktop-active-server";

export function App(props: { desktop?: DesktopHost } = {}) {
  const desktop = props.desktop ?? null;
  return (
    <I18nProvider>
      <DesktopHostContext.Provider value={desktop}>
        {desktop !== null ? <DesktopApp host={desktop} /> : <WebApp />}
        <GlobalErrorDialog />
      </DesktopHostContext.Provider>
    </I18nProvider>
  );
}

function useAppState(desktop: DesktopHost | null = null) {
  const [serversState, setServersState] = useState<ServersState>(() =>
    desktop === null ? loadServersState() : { servers: [], activeId: null },
  );
  const [theme, setTheme] = useState<ThemePreference>(() => loadThemePreference());
  const [accent, setAccent] = useState<AccentPreference>(() => loadAccentPreference());
  const locationHash = useSyncExternalStore(
    subscribeLocationHash,
    locationHashSnapshot,
    locationHashSnapshot,
  );
  const route = useMemo(() => routeFromHash(locationHash), [locationHash]);
  const serversReady = useRef(desktop === null);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    applyAccent(accent);
  }, [accent]);

  useEffect(() => watchSystemTheme(() => loadThemePreference()), []);

  useEffect(() => {
    if (desktop === null) {
      return;
    }
    serversReady.current = false;
    let stale = false;
    void desktop.servers
      .load()
      .then((storedState) => {
        if (!stale) {
          serversReady.current = true;
          setServersState(() => storedState);
        }
      })
      .catch(showError);
    return () => {
      stale = true;
    };
  }, [desktop]);

  const updateServers = (next: ServersState) => {
    if (desktop === null) {
      saveServersState(next);
    } else {
      if (!serversReady.current) {
        showError(new Error("Server storage is not available"));
        return;
      }
      void desktop.servers.save(next).catch(showError);
    }
    setServersState(() => next);
  };

  const updateTheme = (next: ThemePreference) => {
    saveThemePreference(next);
    setTheme(() => next);
  };

  const updateAccent = (next: AccentPreference) => {
    saveAccentPreference(next);
    setAccent(() => next);
  };

  return { serversState, updateServers, theme, updateTheme, accent, updateAccent, route };
}

function WebApp() {
  const state = useAppState();

  const activeServer =
    state.serversState.servers.find((server) => server.id === state.serversState.activeId) ?? null;

  useEffect(() => {
    if (!activeServer && location.hash !== "") {
      history.replaceState(null, "", location.pathname + location.search);
    }
  }, [activeServer]);

  if (!activeServer) {
    return (
      <SetupView
        onCreate={(server) => {
          state.updateServers({
            servers: [...state.serversState.servers, server],
            activeId: server.id,
          });
          navigate("overview");
        }}
        theme={state.theme}
        onThemeChange={state.updateTheme}
        accent={state.accent}
        onAccentChange={state.updateAccent}
      />
    );
  }

  return (
    <Shell
      key={activeServer.id}
      server={activeServer}
      serversState={state.serversState}
      onServersChange={state.updateServers}
      route={state.route}
      theme={state.theme}
      onThemeChange={state.updateTheme}
      accent={state.accent}
      onAccentChange={state.updateAccent}
    />
  );
}

function DesktopApp(props: { host: DesktopHost }) {
  const host = props.host;
  const state = useAppState(host);
  const connection = useDaemonConnection(host);
  const connectionResolvedOnce = useRef(false);
  const connectionHasResolved =
    connection.phase !== "connecting" || connectionResolvedOnce.current;
  useEffect(() => {
    if (connection.phase !== "connecting") {
      connectionResolvedOnce.current = true;
    }
  }, [connection.phase]);
  const [activeId, setActiveId] = useState<string>(
    () => loadStoredString(DESKTOP_ACTIVE_KEY) ?? DESKTOP_LOCAL_SERVER.id,
  );

  useEffect(() => {
    document.body.dataset.platform = host.platform;
  }, [host]);

  const selectServer = (id: string) => {
    saveStoredString(DESKTOP_ACTIVE_KEY, id);
    setActiveId(() => id);
  };

  const servers = state.serversState.servers;
  useEffect(() => {
    if (activeId !== DESKTOP_LOCAL_SERVER.id && !servers.some((server) => server.id === activeId)) {
      saveStoredString(DESKTOP_ACTIVE_KEY, DESKTOP_LOCAL_SERVER.id);
      setActiveId(DESKTOP_LOCAL_SERVER.id);
    }
  }, [activeId, servers]);

  const activeServer =
    state.serversState.servers.find((server) => server.id === activeId) ?? DESKTOP_LOCAL_SERVER;
  const local = activeServer.id === DESKTOP_LOCAL_SERVER.id;

  const picker = (
    <DesktopServerPicker
      serversState={state.serversState}
      localServerId={DESKTOP_LOCAL_SERVER.id}
      activeId={activeServer.id}
      onSelect={selectServer}
    />
  );

  if (local && connection.phase !== "connected") {
    if (!connectionHasResolved) {
      return (
        <div className={styles.desktopRoot}>
          <div className={styles.desktopConnectingView}>
            <Spinner className={styles.connectingSpinner} />
          </div>
        </div>
      );
    }
    return (
      <div className={styles.desktopRoot}>
        <DesktopToolbar window picker={picker} />
        <DesktopSetupView
          host={host}
          state={connection}
          serversState={state.serversState}
          onSelectServer={(server) => selectServer(server.id)}
        />
      </div>
    );
  }

  return (
    <div className={styles.desktopRoot}>
      <Shell
        key={activeServer.id}
        server={activeServer}
        desktopLocal={local}
        desktopPicker={picker}
        onExitRemote={
          local
            ? undefined
            : (alert) => {
                selectServer(DESKTOP_LOCAL_SERVER.id);
                if (alert !== undefined) {
                  showError(alert);
                }
              }
        }
        serversState={state.serversState}
        onServersChange={state.updateServers}
        route={state.route}
        theme={state.theme}
        onThemeChange={state.updateTheme}
        accent={state.accent}
        onAccentChange={state.updateAccent}
      />
    </div>
  );
}

function GlobalErrorDialog() {
  const { t } = useI18n();
  const message = useCurrentError();
  if (message === null) {
    return null;
  }
  return (
    <Dialog onClose={dismissError}>
      <h3>{t("Error")}</h3>
      <p className="dialog-message">{message}</p>
      <div className="row-actions dialog-actions">
        <Button
          onClick={() => {
            void navigator.clipboard.writeText(message).catch(() => {});
          }}
        >
          {t("Copy")}
        </Button>
        <Button variant="primary" onClick={dismissError}>
          {t("Ok")}
        </Button>
      </div>
    </Dialog>
  );
}

interface ShellProps {
  server: Server;
  desktopLocal?: boolean;
  desktopPicker?: React.ReactNode;
  onExitRemote?: (alert?: string) => void;
  serversState: ServersState;
  onServersChange: (state: ServersState) => void;
  route: Route;
  theme: ThemePreference;
  onThemeChange: (theme: ThemePreference) => void;
  accent: AccentPreference;
  onAccentChange: (accent: AccentPreference) => void;
}

function Shell(props: ShellProps) {
  const host = useDesktopHost();
  const [generation, setGeneration] = useState(0);
  const api = useMemo(
    () =>
      host !== null && props.desktopLocal === true
        ? new DaemonApi(props.server, host.transport)
        : new DaemonApi(props.server),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [props.server, host, props.desktopLocal, generation],
  );
  return (
    <DesktopLocalContext.Provider value={host !== null && props.desktopLocal === true}>
      <ApiContext.Provider value={api}>
        <ShellContent {...props} onRetry={() => setGeneration(generation + 1)} />
      </ApiContext.Provider>
    </DesktopLocalContext.Provider>
  );
}

function ShellContent(props: ShellProps & { onRetry: () => void }) {
  const api = useApi();
  const host = useDesktopHost();
  const localHost = useLocalDesktopHost();
  const { t, language } = useI18n();
  const route = props.route;
  const serviceStatus = useStream(api.serviceStatus);
  const groups = useStream(api.groups);
  const [menuOpen, setMenuOpen] = useState(false);
  const [leadSlot, setLeadSlot] = useState<HTMLElement | null>(null);
  const [endSlot, setEndSlot] = useState<HTMLElement | null>(null);

  const lostError = useStreamOutage(
    serviceStatus,
    isTerminalCode(serviceStatus.errorCode) || serviceStatus.data.status === null,
  );

  const onExitRemote = props.onExitRemote;
  useRemoteSession(
    host !== null && props.desktopLocal !== true ? serviceStatus : null,
    (failure) => {
      onExitRemote?.(
        t(
          failure.hadConnected
            ? "Disconnected from remote server {name}"
            : "Failed to connect to remote server {name}",
          { name: serverDisplayName(props.server) },
        ) + (failure.message !== "" ? `\n${failure.message}` : ""),
      );
    },
  );

  useEffect(() => {
    const kick = () => {
      if (!document.hidden) {
        api.reconnectNow();
      }
    };
    document.addEventListener("visibilitychange", kick);
    window.addEventListener("pageshow", kick);
    window.addEventListener("online", kick);
    return () => {
      document.removeEventListener("visibilitychange", kick);
      window.removeEventListener("pageshow", kick);
      window.removeEventListener("online", kick);
    };
  }, [api]);

  const reachable = serviceStatus.phase === "active";
  const serverInfo = useUnaryOnce(() => api.serverInfo(), reachable);
  const capabilities = useMemo(
    () => makeCapabilities(serverInfo?.apiVersion ?? null),
    [serverInfo],
  );

  useEffect(() => {
    setMenuOpen(false);
  }, [route]);

  const started = serviceStatus.data.status?.status === ServiceStatus_Type.STARTED;
  const hasGroups = started && groups.data.loaded && groups.data.groups.length > 0;
  const known = serviceStatus.phase !== "connecting" || serviceStatus.data.status !== null;

  const groupsKnown = groups.data.loaded || groups.phase === "error";
  useEffect(() => {
    if (!known) {
      return;
    }
    const invisible =
      (route.page === "groups" && (!started || (groupsKnown && !hasGroups))) ||
      (route.page === "connections" && !started) ||
      (isStartedOnlyToolsSubpage(route.page) && !started) ||
      (route.page === "tools/usbip" && capabilities.ready && !capabilities.supports("usbip")) ||
      (isLocalReportsPage(route.page) && localHost === null);
    if (invisible) {
      navigate(
        isStartedOnlyToolsSubpage(route.page) || isLocalReportsPage(route.page)
          ? "tools"
          : "overview",
      );
    }
  }, [known, started, groupsKnown, hasGroups, route, capabilities, localHost]);

  if (lostError !== null && host === null) {
    return (
      <ConnectionErrorView
        server={props.server}
        error={lostError}
        reconnecting={serviceStatus.phase === "connecting"}
        onRetry={props.onRetry}
        serversState={props.serversState}
        onServersChange={props.onServersChange}
      />
    );
  }

  if (serviceStatus.data.status === null) {
    if (host !== null) {
      return (
        <div className={styles.desktopConnectingView}>
          <Spinner className={styles.connectingSpinner} />
        </div>
      );
    }
    return (
      <div className={styles.connectingView}>
        <Brand className={styles.connectingBrand} />
        <Spinner className={styles.connectingSpinner} />
      </div>
    );
  }

  if (route.page === "tools/tailscale/ssh") {
    return (
      <TailscaleSSHView
        key={`${route.tag}/${route.peerID}/${route.username}/${route.terminalType}`}
        tag={route.tag}
        peerID={route.peerID}
        username={route.username}
        terminalType={route.terminalType}
      />
    );
  }

  const navItem = (page: string, title: string, icon: IconName, active: boolean) => (
    <button
      type="button"
      key={page}
      className={cx(styles.navItem, active && styles.active)}
      onClick={() => {
        setMenuOpen(false);
        navigate(page);
      }}
    >
      <Icon name={icon} />
      {title}
    </button>
  );

  const mainPages = (
    <>
      {navItem("logs", t("Logs"), "text_snippet", route.page === "logs")}
      {navItem("tools", t("Tools"), "terminal", route.page.startsWith("tools"))}
      {navItem("settings", t("Settings"), "settings", route.page.startsWith("settings"))}
    </>
  );

  const mainContent = (
    <main className={styles.content}>
      {route.page === "overview" && <OverviewView />}
      {route.page === "groups" && <GroupsView />}
      {route.page === "connections" && <ConnectionsView />}
      {route.page === "logs" && <LogsView />}
      {route.page === "tools" && <ToolsView />}
      {route.page === "tools/network-quality" && <NetworkQualityView />}
      {route.page === "tools/stun" && <STUNTestView />}
      {route.page === "tools/tailscale" && <TailscaleEndpointView tag={route.tag} />}
      {route.page === "tools/usbip" && <UsbipView tag={route.tag} />}
      {route.page === "tools/crash-reports" && <CrashReportListView />}
      {route.page === "tools/crash-reports/detail" && (
        <CrashReportDetailView name={route.name} crashedAt={route.crashedAt} />
      )}
      {route.page === "tools/crash-reports/file" && (
        <CrashReportFileView name={route.name} file={route.file} crashedAt={route.crashedAt} />
      )}
      {route.page === "tools/oom-reports" && <OOMReportListView />}
      {route.page === "tools/oom-reports/detail" && (
        <OOMReportDetailView name={route.name} recordedAt={route.recordedAt} />
      )}
      {route.page === "tools/oom-reports/file" && (
        <OOMReportFileView name={route.name} file={route.file} recordedAt={route.recordedAt} />
      )}
      {route.page === "settings" && <SettingsView />}
      {route.page === "settings/app" && (
        <AppSettingsView
          theme={props.theme}
          onThemeChange={props.onThemeChange}
          accent={props.accent}
          onAccentChange={props.onAccentChange}
        />
      )}
      {route.page === "settings/core" && <CoreView />}
      {route.page === "settings/preferences" && (
        <PreferencesView
          theme={props.theme}
          onThemeChange={props.onThemeChange}
          accent={props.accent}
          onAccentChange={props.onAccentChange}
        />
      )}
      {route.page === "settings/preferences/terminal" && <TerminalConfigurationView />}
      {route.page === "settings/preferences/terminal/theme" && (
        <TerminalThemePickerView scheme={route.scheme} />
      )}
      {route.page === "settings/preferences/terminal/custom" && (
        <TerminalThemeEditorView scheme={route.scheme} />
      )}
      {route.page === "settings/servers" && (
        <ServersView serversState={props.serversState} onServersChange={props.onServersChange} />
      )}
    </main>
  );

  return (
    <CapabilitiesContext.Provider value={capabilities}>
      <div className={styles.app}>
        {host === null && (
          <header className={styles.mobileTopbar}>
            <IconButton
              aria-label={t("Toggle navigation")}
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen(!menuOpen)}
            >
              <Icon name={menuOpen ? "close" : "menu"} size={18} />
            </IconButton>
            <div className={styles.mobileTopbarBrand}>sing-box</div>
          </header>
        )}
        {menuOpen && (
          <button
            type="button"
            className={styles.sidebarScrim}
            aria-label={t("Close")}
            onClick={() => setMenuOpen(false)}
          />
        )}
        {host !== null ? (
          <nav className={styles.sidebar}>
            <div className={styles.sidebarTitlebar} />
            <div className={styles.sidebarBrand}>
              sing-box
              {serverInfo && <span className={styles.sidebarBrandVersion}>{serverInfo.version}</span>}
            </div>
            {started ? (
              <>
                {navItem("overview", t("Overview"), "dashboard", route.page === "overview")}
                {hasGroups && navItem("groups", t("Groups"), "folder", route.page === "groups")}
                {navItem("connections", t("Connections"), "swap_vert", route.page === "connections")}
              </>
            ) : (
              navItem("overview", t("Dashboard"), "dashboard", route.page === "overview")
            )}
            {mainPages}
          </nav>
        ) : (
          <nav className={cx(styles.sidebar, menuOpen && styles.open)}>
            <div className={styles.sidebarBrand}>
              sing-box
              {serverInfo && <span className={styles.sidebarBrandVersion}>{serverInfo.version}</span>}
            </div>
            {navItem("overview", t("Overview"), "dashboard", route.page === "overview")}
            {hasGroups && navItem("groups", t("Groups"), "folder", route.page === "groups")}
            {started && navItem("connections", t("Connections"), "swap_vert", route.page === "connections")}
            {mainPages}
            <ServerPicker
              serversState={props.serversState}
              onServersChange={props.onServersChange}
              connected={reachable}
              started={started}
            />
          </nav>
        )}
        {host !== null ? (
          <div className={styles.contentColumn}>
            <DesktopToolbar
              title={routeTitle(route, t, language)}
              picker={props.desktopPicker}
              controls={
                props.desktopLocal === true ? (
                  <DesktopServiceControls host={host} />
                ) : onExitRemote !== undefined ? (
                  <DesktopRemoteControls onDisconnect={() => onExitRemote()} />
                ) : undefined
              }
              leadRef={setLeadSlot}
              endRef={setEndSlot}
            />
            <ToolbarSlotsProvider lead={leadSlot} end={endSlot}>
              {mainContent}
            </ToolbarSlotsProvider>
          </div>
        ) : (
          mainContent
        )}
        {serviceStatus.phase !== "active" && (
          <div className={styles.reconnectPill} role="status">
            <Spinner />
            {t("Reconnecting...")}
          </div>
        )}
        {started && <DeprecatedWarningsGate />}
        {localHost !== null && <ImportRemoteProfileDialog host={localHost} />}
        {localHost !== null && <ImportProfileFileDialog host={localHost} />}
      </div>
    </CapabilitiesContext.Provider>
  );
}

function ServerPicker(props: {
  serversState: ServersState;
  onServersChange: (state: ServersState) => void;
  connected: boolean;
  started: boolean;
}) {
  const { t } = useI18n();
  const { servers, activeId } = props.serversState;
  const active = servers.find((server) => server.id === activeId);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useDismiss(ref, open, () => setOpen(false));

  if (!active) {
    return null;
  }

  return (
    <div className={styles.serverPicker} ref={ref}>
      <button type="button" className={styles.serverPickerButton} aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className={styles.serverPickerText}>
          <span className={styles.serverPickerLine}>
            <StateDot tone={props.connected ? "good" : undefined} className={styles.serverDot} />
            <span className={styles.serverName}>{serverDisplayName(active)}</span>
          </span>
          {props.started && <ServerUptime />}
        </span>
        <Icon name="unfold_more" size={13} />
      </button>
      {open && (
        <div className="menu open-up">
          {servers.map((server) => (
            <button
              type="button"
              key={server.id}
              className="menu-item"
              onClick={() => {
                setOpen(false);
                if (server.id !== activeId) {
                  props.onServersChange({ ...props.serversState, activeId: server.id });
                }
              }}
            >
              <span className="menu-check">{server.id === activeId && <Icon name="check" size={13} />}</span>
              {serverDisplayName(server)}
            </button>
          ))}
          <div className="menu-divider" />
          <button
            type="button"
            className="menu-item"
            onClick={() => {
              setOpen(false);
              navigate("settings/servers");
            }}
          >
            <span className="menu-check">
              <Icon name="settings" size={13} />
            </span>
            {t("Manage servers...")}
          </button>
        </div>
      )}
    </div>
  );
}

function ServerUptime() {
  const api = useApi();
  const { t, language } = useI18n();
  const now = useNow();
  const startedAt = useUnaryOnce(() => api.getStartedAt());

  if (startedAt === null) {
    return null;
  }
  return (
    <span className={styles.serverUptime} title={`${t("Uptime")} — ${formatDateTime(startedAt, language)}`}>
      <Icon name="power_settings_new" size={10} />
      {formatUptime(startedAt, now)}
    </span>
  );
}

function DeprecatedWarningsGate() {
  const api = useApi();
  const warnings = useUnaryOnce(() => api.getDeprecatedWarnings());
  const [index, setIndex] = useState(0);
  const [visible, setVisible] = useState(true);

  if (loadDisableDeprecatedWarnings()) {
    return null;
  }

  const current = warnings?.[index];
  if (!current || !visible) {
    return null;
  }

  return (
    <DeprecatedWarningDialog
      warning={current}
      onDismiss={() => {
        setVisible(false);
        setTimeout(() => {
          setIndex((value) => value + 1);
          setVisible(true);
        }, 300);
      }}
    />
  );
}

function DeprecatedWarningDialog(props: { warning: DeprecatedWarning; onDismiss: () => void }) {
  const { t } = useI18n();
  return (
    <Dialog onClose={props.onDismiss}>
      <h3>{t("Deprecated Warning")}</h3>
      <p className="dialog-message">{props.warning.message}</p>
      <div className="row-actions dialog-actions">
        <Button onClick={props.onDismiss}>
          {t("Ok")}
        </Button>
        {isHttpUrl(props.warning.migrationLink) && (
          <Button
            variant="primary"
            href={props.warning.migrationLink}
            target="_blank"
            rel="noreferrer"
            onClick={props.onDismiss}
          >
            {t("Documentation")}
          </Button>
        )}
      </div>
    </Dialog>
  );
}
