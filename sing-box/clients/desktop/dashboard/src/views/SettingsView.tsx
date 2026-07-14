import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import {
  createServerId,
  normalizeServerUrl,
  removeServer,
  serverDisplayName,
  upsertServer,
  type Server,
  type ServersState,
} from "../api/config";
import { formatBytes } from "../api/format";
import { useStream } from "../api/stream";
import { navigate, useApi, type AccentPreference, type ThemePreference } from "../app/context";
import { useDesktopHost, useLocalDesktopHost } from "../app/desktop";
import type { DesktopHost, DesktopSettingsState } from "../app/desktop";
import {
  loadDisableDeprecatedWarnings,
  saveDisableDeprecatedWarnings,
} from "../app/deprecatedWarnings";
import { showError } from "../app/errorStore";
import { ServiceStatus_Type } from "../gen/daemon/started_service_pb";
import { LanguageSelect, useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import { PageHeader } from "../components/PageHeader";
import { ReachabilityIndicator, useServerReachability } from "../components/ReachabilityIndicator";
import { Button, Dialog, Field, IconButton, MenuItem, MenuLink, NavRow, SecretInput, Select, Spinner, ThemeMenu, ThemeSelect, useContextMenu } from "../components/ui";
import {
  DEFAULT_DARK_THEME_NAME,
  DEFAULT_LIGHT_THEME_NAME,
  loadTerminalConfig,
  saveTerminalConfig,
  type TerminalConfig,
} from "../lib/tailscaleSSH";
import { parseCustomTheme, type Scheme, type TerminalThemeEntry } from "../lib/terminalTheme";
import styles from "./SettingsView.module.css";
import { cx } from "../lib/cx";

export function SettingsView() {
  const { t } = useI18n();
  const host = useDesktopHost();
  const localHost = useLocalDesktopHost();

  return (
    <div className="page">
      <PageHeader title={t("Settings")} />
      <div className="settings-stack">
        <div className="nav-list">
          {host === null && (
            <NavRow
              icon="tune"
              title={t("Preferences")}
              onClick={() => navigate("settings/preferences")}
            />
          )}
          {host !== null && (
            <NavRow icon="info" title={t("App")} onClick={() => navigate("settings/app")} />
          )}
          {localHost !== null && (
            <NavRow
              icon="memory"
              title={t("Core")}
              onClick={() => navigate("settings/core")}
            />
          )}
          <NavRow
            icon="dns"
            title={host !== null ? t("Remote Control") : t("Servers")}
            onClick={() => navigate("settings/servers")}
          />
        </div>
        <div>
          <div className="list-section-title">{t("About")}</div>
          <div className="nav-list">
            <NavRow
              icon="description"
              title={t("Documentation")}
              href="https://sing-box.sagernet.org"
              contextMenu={
                host !== null ? (
                  <>
                    <MenuLink href="https://sing-box.sagernet.org/changelog/">
                      {t("Changelog")}
                    </MenuLink>
                    <MenuLink href="https://sing-box.sagernet.org/configuration/">
                      {t("Configuration")}
                    </MenuLink>
                  </>
                ) : undefined
              }
            />
            <NavRow
              icon="code"
              title={t("Source Code")}
              href={
                host !== null
                  ? "https://github.com/SagerNet/sing-box"
                  : "https://github.com/SagerNet/sing-box-dashboard"
              }
              contextMenu={
                host !== null ? (
                  <MenuLink href="https://github.com/SagerNet/sing-box/releases">
                    {t("Releases")}
                  </MenuLink>
                ) : undefined
              }
            />
          </div>
        </div>
      </div>
    </div>
  );
}

export function AppSettingsView(props: {
  theme: ThemePreference;
  onThemeChange: (theme: ThemePreference) => void;
  accent: AccentPreference;
  onAccentChange: (accent: AccentPreference) => void;
}) {
  const host = useDesktopHost();
  if (host === null) {
    return null;
  }
  return <AppSettingsContent host={host} {...props} />;
}

function AppSettingsContent({
  host,
  ...preferences
}: {
  host: DesktopHost;
  theme: ThemePreference;
  onThemeChange: (theme: ThemePreference) => void;
  accent: AccentPreference;
  onAccentChange: (accent: AccentPreference) => void;
}) {
  const { t } = useI18n();
  const [settings, setSettings] = useState<DesktopSettingsState | null>(null);
  const [cacheSize, setCacheSize] = useState<number | null>(null);
  const [clearing, setClearing] = useState(false);
  const [savingOpenAtLogin, setSavingOpenAtLogin] = useState(false);

  useEffect(() => {
    let stale = false;
    host.settings
      .get()
      .then((value) => {
        if (!stale) {
          setSettings(() => value);
        }
      })
      .catch(showError);
    host.settings
      .cacheSize()
      .then((value) => {
        if (!stale) {
          setCacheSize(() => value);
        }
      })
      .catch(showError);
    return () => {
      stale = true;
    };
  }, [host]);

  const clearCache = () => {
    setClearing(true);
    host.settings
      .clearCache()
      .then(() => host.settings.cacheSize())
      .then(setCacheSize)
      .catch(showError)
      .finally(() => setClearing(false));
  };

  const refreshOpenAtLogin = () =>
    host.settings.get().then(({ openAtLogin }) => {
      setSettings((current) =>
        current === null ? current : { ...current, openAtLogin },
      );
    });

  return (
    <div className="page">
      <SettingsPageHeader title={t("App")} />
      <div className="settings-stack">
        {settings === null ? (
          <Spinner />
        ) : (
          <>
            <div className={styles.settingsList}>
              <div className="settings-row">
                <span className="settings-row-label">{t("Language")}</span>
                <LanguageSelect />
              </div>
              <div className="settings-row">
                <span className="settings-row-label">{t("Appearance")}</span>
                <ThemeSelect theme={preferences.theme} onChange={preferences.onThemeChange} />
              </div>
              <div className="settings-row">
                <span className="settings-row-label">{t("Theme")}</span>
                <ThemeMenu accent={preferences.accent} onChange={preferences.onAccentChange} />
              </div>
              <div className="settings-row">
                <span className="settings-row-label">{t("Start At Login")}</span>
                <button
                  type="button"
                  className={settings.openAtLogin ? "switch on" : "switch"}
                  role="switch"
                  aria-checked={settings.openAtLogin}
                  aria-label={t("Start At Login")}
                  disabled={savingOpenAtLogin}
                  onClick={() => {
                    const value = !settings.openAtLogin;
                    setSettings({ ...settings, openAtLogin: value });
                    setSavingOpenAtLogin(true);
                    host.settings
                      .setOpenAtLogin(value)
                      .then(refreshOpenAtLogin)
                      .catch((error) => {
                        showError(error);
                        return refreshOpenAtLogin().catch(showError);
                      })
                      .finally(() => setSavingOpenAtLogin(false));
                  }}
                />
              </div>
              <div className="settings-row">
                <span className="settings-row-label">{t("Enable Tray")}</span>
                <button
                  type="button"
                  className={settings.trayEnabled ? "switch on" : "switch"}
                  role="switch"
                  aria-checked={settings.trayEnabled}
                  aria-label={t("Enable Tray")}
                  onClick={() => {
                    const value = !settings.trayEnabled;
                    setSettings({
                      ...settings,
                      trayEnabled: value,
                    });
                    host.settings.setTrayEnabled(value).catch(showError);
                  }}
                />
              </div>
              {settings.trayEnabled && (
                <div className="settings-row">
                  <span className="settings-row-label">{t("Keep Tray in Background")}</span>
                  <button
                    type="button"
                    className={settings.trayInBackground ? "switch on" : "switch"}
                    role="switch"
                    aria-checked={settings.trayInBackground}
                    aria-label={t("Keep Tray in Background")}
                    onClick={() => {
                      const value = !settings.trayInBackground;
                      setSettings({ ...settings, trayInBackground: value });
                      host.settings.setTrayInBackground(value).catch(showError);
                    }}
                  />
                </div>
              )}
              <div className="settings-row">
                <span className="settings-row-label">{t("Cache Size")}</span>
                {cacheSize === null ? (
                  <Spinner />
                ) : (
                  <span className="nav-row-detail">{formatBytes(cacheSize)}</span>
                )}
              </div>
              {cacheSize !== null && cacheSize > 0 && (
                <button
                  type="button"
                  className={cx("settings-row", styles.destructiveRow)}
                  disabled={clearing}
                  onClick={clearCache}
                >
                  <span className="settings-row-label">{t("Clear Cache")}</span>
                  {clearing && <Spinner />}
                </button>
              )}
            </div>
            <div>
              <div className="list-section-title">Tailscale</div>
              <div className="nav-list">
                <NavRow
                  icon="terminal"
                  title={t("Terminal Configuration")}
                  onClick={() => navigate("settings/preferences/terminal")}
                />
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function SettingsPageHeader(props: {
  title: string;
  action?: ReactNode;
  back?: string;
  backLabel?: string;
}) {
  const { t } = useI18n();
  const back = props.back ?? "settings";
  return (
    <PageHeader
      title={props.title}
      actions={props.action}
      back={{ label: props.backLabel ?? t("Settings"), onClick: () => navigate(back) }}
    />
  );
}

export function CoreView() {
  const host = useLocalDesktopHost();
  if (host === null) {
    return null;
  }
  return <CoreViewContent host={host} />;
}

function CoreViewContent({ host }: { host: DesktopHost }) {
  const { t } = useI18n();
  const api = useApi();
  const serviceStatus = useStream(api.serviceStatus);
  const [coreVersion, setCoreVersion] = useState<string | null>(null);
  const [dataSize, setDataSize] = useState<number | "unavailable" | null>(null);
  const [disableWarnings, setDisableWarnings] = useState(loadDisableDeprecatedWarnings);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const copyMenu = useContextMenu(
    <MenuItem
      icon="content_copy"
      onSelect={() => {
        void navigator.clipboard.writeText(coreVersion ?? "").catch(showError);
      }}
    >
      {t("Copy")}
    </MenuItem>,
  );

  const running =
    serviceStatus.data.status?.status === ServiceStatus_Type.STARTED ||
    serviceStatus.data.status?.status === ServiceStatus_Type.STARTING;

  const loadInfo = useCallback(() => {
    setCoreVersion(null);
    host.core
      .info()
      .then((value) => setCoreVersion(value.version))
      .catch(showError);
  }, [host]);

  useEffect(() => {
    loadInfo();
  }, [loadInfo]);

  const refreshSize = useCallback(() => {
    setDataSize(null);
    host.core
      .workingDirectory()
      .then((value) => setDataSize(value.size))
      .catch(() => setDataSize("unavailable"));
  }, [host]);

  useEffect(() => {
    refreshSize();
  }, [refreshSize]);

  const destroy = () => {
    setBusy(true);
    host.core
      .destroyWorkingDirectory()
      .then(() => {
        setConfirming(false);
        loadInfo();
        refreshSize();
      })
      .catch(showError)
      .finally(() => setBusy(false));
  };

  return (
    <div className="page">
      <SettingsPageHeader title={t("Core")} />
      <div className="settings-stack">
        {coreVersion === null ? (
          <Spinner />
        ) : (
          <>
            <div className={styles.settingsList}>
              <div className="settings-row" onContextMenu={copyMenu.onContextMenu}>
                <span className="settings-row-label">{t("Version")}</span>
                <span className="nav-row-detail">{coreVersion}</span>
                {copyMenu.element}
              </div>
              <div className="settings-row">
                <span className="settings-row-label">{t("Data Size")}</span>
                {dataSize === null ? (
                  <Spinner />
                ) : dataSize === "unavailable" ? (
                  <span className={styles.fieldError}>{t("Unavailable")}</span>
                ) : (
                  <span className="nav-row-detail">{formatBytes(dataSize)}</span>
                )}
              </div>
            </div>
            {coreVersion.includes("-") && (
              <div>
                <div className="list-section-title">{t("Beta Settings")}</div>
                <div className={styles.settingsList}>
                  <div className="settings-row">
                    <div className={styles.rowText}>
                      <span className="settings-row-label">{t("Disable Deprecated Warnings")}</span>
                      <span className="hint">
                        {t("Do not show warnings about usages of deprecated features.")}
                      </span>
                    </div>
                    <button
                      type="button"
                      className={disableWarnings ? "switch on" : "switch"}
                      role="switch"
                      aria-checked={disableWarnings}
                      aria-label={t("Disable Deprecated Warnings")}
                      onClick={() => {
                        const value = !disableWarnings;
                        setDisableWarnings(value);
                        saveDisableDeprecatedWarnings(value);
                      }}
                    />
                  </div>
                </div>
              </div>
            )}
            <div>
              <div className="list-section-title">{t("Working Directory")}</div>
              <div className={styles.settingsList}>
                <button
                  type="button"
                  className={cx("settings-row", styles.destructiveRow)}
                  disabled={busy}
                  onClick={() => (running ? setConfirming(true) : destroy())}
                >
                  <span className="settings-row-label">{t("Destroy")}</span>
                  {busy && !confirming && <Spinner />}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
      {confirming && (
        <Dialog onClose={() => (busy ? undefined : setConfirming(false))}>
          <h3>{t("Service is Running")}</h3>
          <p className="dialog-message">
            {t("The service must be stopped before destroying the working directory.")}
          </p>
          <div className="row-actions dialog-actions">
            <Button onClick={() => setConfirming(false)} disabled={busy}>
              {t("Cancel")}
            </Button>
            <Button variant="danger" onClick={destroy} disabled={busy}>
              {busy ? <Spinner /> : t("Stop Service and Continue")}
            </Button>
          </div>
        </Dialog>
      )}
    </div>
  );
}

export function PreferencesView(props: {
  theme: ThemePreference;
  onThemeChange: (theme: ThemePreference) => void;
  accent: AccentPreference;
  onAccentChange: (accent: AccentPreference) => void;
}) {
  const { t } = useI18n();
  return (
    <div className="page">
      <SettingsPageHeader title={t("Preferences")} />
      <div className="settings-stack">
        <div className={styles.settingsList}>
          <div className="settings-row">
            <Icon name="brightness_auto" size={15} />
            <span className="settings-row-label">{t("Appearance")}</span>
            <ThemeSelect theme={props.theme} onChange={props.onThemeChange} />
          </div>
          <div className="settings-row">
            <Icon name="palette" size={15} />
            <span className="settings-row-label">{t("Theme")}</span>
            <ThemeMenu accent={props.accent} onChange={props.onAccentChange} />
          </div>
          <div className="settings-row">
            <Icon name="language" size={15} />
            <span className="settings-row-label">{t("Language")}</span>
            <LanguageSelect />
          </div>
        </div>
        <div>
          <div className="list-section-title">Tailscale</div>
          <div className="nav-list">
            <NavRow
              icon="terminal"
              title={t("Terminal Configuration")}
              onClick={() => navigate("settings/preferences/terminal")}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

const FONT_SIZES = [8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32];

const FONT_FAMILIES = [
  "Menlo",
  "Monaco",
  "SF Mono",
  "Consolas",
  "Cascadia Code",
  "Fira Code",
  "JetBrains Mono",
  "Source Code Pro",
  "IBM Plex Mono",
  "Roboto Mono",
  "Ubuntu Mono",
  "Courier New",
];

const CUSTOM_THEME_PLACEHOLDER = `{
  "background": "#1e1e1e",
  "foreground": "#d4d4d4",
  "cursor": "#d4d4d4",
  "selectionBackground": "#264f78"
}`;

function ThemeSchemeSection(props: {
  scheme: Scheme;
  name: string;
  custom: string;
  onChange: (patch: Partial<TerminalConfig>) => void;
}) {
  const { t } = useI18n();
  const { scheme, name, custom } = props;
  const isDark = scheme === "dark";
  const isCustom = name === "";
  const remembered = useRef(
    name || (isDark ? DEFAULT_DARK_THEME_NAME : DEFAULT_LIGHT_THEME_NAME),
  );
  const invalid = isCustom && custom.trim() !== "" && parseCustomTheme(custom) === null;

  const setName = (value: string) => {
    if (value !== "") {
      remembered.current = value;
    }
    props.onChange(isDark ? { darkThemeName: value } : { lightThemeName: value });
  };

  return (
    <div>
      <div className="list-section-title">{isDark ? t("Dark") : t("Light")}</div>
      <div className={styles.settingsList}>
        <button
          type="button"
          className="settings-row"
          disabled={isCustom}
          onClick={() => navigate(`settings/preferences/terminal/theme/${scheme}`)}
        >
          <span className="settings-row-label">{t("Theme")}</span>
          <span className="nav-row-detail">{isCustom ? t("Custom theme") : name}</span>
          <span className="settings-row-chevron">
            <Icon name="keyboard_arrow_right" size={14} />
          </span>
        </button>
        <div className="settings-row">
          <span className="settings-row-label">{t("Custom theme")}</span>
          <button
            type="button"
            className={isCustom ? "switch on" : "switch"}
            role="switch"
            aria-checked={isCustom}
            aria-label={t("Custom theme")}
            onClick={() => setName(isCustom ? remembered.current : "")}
          />
        </div>
        {isCustom && (
          <button
            type="button"
            className="settings-row"
            onClick={() => navigate(`settings/preferences/terminal/custom/${scheme}`)}
          >
            <span className="settings-row-label">{t("Edit custom theme")}</span>
            {invalid && <span className={styles.fieldError}>{t("Invalid theme JSON")}</span>}
            <span className="settings-row-chevron">
              <Icon name="keyboard_arrow_right" size={14} />
            </span>
          </button>
        )}
      </div>
    </div>
  );
}

export function TerminalConfigurationView() {
  const { t } = useI18n();
  const host = useDesktopHost();
  const [config, setConfig] = useState<TerminalConfig>(loadTerminalConfig);

  const update = (patch: Partial<TerminalConfig>) => {
    const next = { ...config, ...patch };
    setConfig(next);
    saveTerminalConfig(next);
  };

  return (
    <div className="page">
      <SettingsPageHeader
        title={t("Terminal Configuration")}
        back={host !== null ? "settings/app" : "settings/preferences"}
        backLabel={host !== null ? t("App") : t("Preferences")}
      />
      <div className="settings-stack">
        <ThemeSchemeSection
          scheme="light"
          name={config.lightThemeName}
          custom={config.lightThemeCustom}
          onChange={update}
        />
        <ThemeSchemeSection
          scheme="dark"
          name={config.darkThemeName}
          custom={config.darkThemeCustom}
          onChange={update}
        />
        <div>
          <div className="list-section-title">{t("Font")}</div>
          <div className={styles.settingsList}>
            <div className="settings-row">
              <span className="settings-row-label">{t("Font family")}</span>
              <Select
                inline
                options={[
                  { value: "", label: t("Default") },
                  ...FONT_FAMILIES.map((family) => ({ value: family, label: family })),
                ]}
                value={config.fontFamily}
                onChange={(fontFamily) => update({ fontFamily })}
              />
            </div>
            <div className="settings-row">
              <span className="settings-row-label">{t("Font size")}</span>
              <Select
                inline
                options={FONT_SIZES.map((size) => ({ value: size, label: String(size) }))}
                value={config.fontSize}
                onChange={(fontSize) => update({ fontSize })}
              />
            </div>
          </div>
        </div>
        <div>
          <div className="list-section-title">{t("Symbol Bar")}</div>
          <div className={styles.settingsList}>
            <div className="settings-row">
              <span className="settings-row-label">{t("Always show")}</span>
              <button
                type="button"
                className={config.symbolBarAlwaysShow ? "switch on" : "switch"}
                role="switch"
                aria-checked={config.symbolBarAlwaysShow}
                aria-label={t("Always show")}
                onClick={() => update({ symbolBarAlwaysShow: !config.symbolBarAlwaysShow })}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export function TerminalThemeEditorView(props: { scheme: Scheme }) {
  const { t } = useI18n();
  const isDark = props.scheme === "dark";
  const [value, setValue] = useState(() => {
    const config = loadTerminalConfig();
    return isDark ? config.darkThemeCustom : config.lightThemeCustom;
  });
  const invalid = value.trim() !== "" && parseCustomTheme(value) === null;

  return (
    <div className="page">
      <SettingsPageHeader
        title={t("Custom theme")}
        back="settings/preferences/terminal"
        backLabel={t("Terminal Configuration")}
      />
      <div className="settings-stack">
        <textarea
          className={cx("input", styles.themeEditor)}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          placeholder={CUSTOM_THEME_PLACEHOLDER}
          value={value}
          onChange={(event) => {
            const next = event.target.value;
            setValue(() => next);
            const latest = loadTerminalConfig();
            saveTerminalConfig(
              isDark ? { ...latest, darkThemeCustom: next } : { ...latest, lightThemeCustom: next },
            );
          }}
        />
        {invalid && <span className={styles.fieldError}>{t("Invalid theme JSON")}</span>}
        <p className={styles.themeEditorNote}>
          {t("Colors use the xterm.js theme format.")}{" "}
          <a
            href="https://xtermjs.org/docs/api/terminal/interfaces/itheme/"
            target="_blank"
            rel="noreferrer"
          >
            {t("Learn more")}
          </a>
        </p>
      </div>
    </div>
  );
}

function ThemePreview({ theme }: { theme: TerminalThemeEntry["theme"] }) {
  const fg = theme.foreground;
  const color = (value?: string) => value ?? fg;
  return (
    <span className={styles.themePreview} style={{ background: theme.background, color: fg }}>
      <span className={styles.themePreviewLine}>
        <span style={{ color: color(theme.green) }}>➜</span>{" "}
        <span style={{ color: color(theme.cyan) }}>~/project</span>{" "}
        <span style={{ color: color(theme.blue) }}>git:(</span>
        <span style={{ color: color(theme.red) }}>main</span>
        <span style={{ color: color(theme.blue) }}>)</span>
      </span>
      <span className={styles.themePreviewLine}>
        <span style={{ color: color(theme.yellow) }}>$</span>{" "}
        <span>npm</span> <span style={{ color: color(theme.magenta) }}>run</span>{" "}
        <span style={{ color: color(theme.green) }}>build</span>
      </span>
    </span>
  );
}

export function TerminalThemePickerView(props: { scheme: Scheme }) {
  const { t } = useI18n();
  const current =
    props.scheme === "dark"
      ? loadTerminalConfig().darkThemeName
      : loadTerminalConfig().lightThemeName;
  const [query, setQuery] = useState("");
  const [themes, setThemes] = useState<TerminalThemeEntry[] | null>(null);

  useEffect(() => {
    let active = true;
    void import("../lib/terminalThemes").then((module) => {
      if (active) {
        setThemes(
          module.TERMINAL_THEMES.filter((entry) => entry.isDark === (props.scheme === "dark")),
        );
      }
    });
    return () => {
      active = false;
    };
  }, [props.scheme]);

  const filtered = useMemo(() => {
    if (!themes) {
      return [];
    }
    const needle = query.trim().toLowerCase();
    return needle === ""
      ? themes
      : themes.filter((entry) => entry.name.toLowerCase().includes(needle));
  }, [themes, query]);

  const select = (name: string) => {
    const latest = loadTerminalConfig();
    saveTerminalConfig(
      props.scheme === "dark"
        ? { ...latest, darkThemeName: name }
        : { ...latest, lightThemeName: name },
    );
    navigate("settings/preferences/terminal");
  };

  return (
    <div className="page">
      <SettingsPageHeader
        title={props.scheme === "dark" ? t("Dark") : t("Light")}
        back="settings/preferences/terminal"
        backLabel={t("Terminal Configuration")}
      />
      <div className="settings-stack">
        <div className="search-input">
          <Icon name="search" size={14} />
          <input
            className="input"
            placeholder={t("Search themes")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        {themes === null ? (
          <div className={styles.themePickerLoading}>
            <Spinner />
          </div>
        ) : (
          <div className={styles.settingsList}>
            {filtered.map((entry) => (
              <button
                type="button"
                key={entry.name}
                className={cx("settings-row", styles.themePickerRow)}
                onClick={() => select(entry.name)}
              >
                <ThemePreview theme={entry.theme} />
                <span className="settings-row-label">{entry.name}</span>
                {entry.name === current && <Icon name="check" size={16} />}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export function ServersView(props: {
  serversState: ServersState;
  onServersChange: (state: ServersState) => void;
}) {
  const { t } = useI18n();
  const host = useDesktopHost();
  const { servers } = props.serversState;
  const [editing, setEditing] = useState<Server | "new" | null>(null);

  const saveServer = (server: Server) => {
    props.onServersChange(upsertServer(props.serversState, server));
    setEditing(null);
  };

  const deleteServer = (id: string) => {
    props.onServersChange(removeServer(props.serversState, id));
    setEditing(null);
  };

  return (
    <div className="page">
      <SettingsPageHeader
        title={host !== null ? t("Remote Control") : t("Servers")}
        action={
          <IconButton
            aria-label={t("New Server")}
            title={t("New Server")}
            onClick={() => setEditing("new")}
          >
            <Icon name="add" size={18} />
          </IconButton>
        }
      />
      <div>
        {host !== null && <div className="list-section-title">{t("Servers")}</div>}
        <div className="nav-list">
          {servers.length === 0 ? (
            <div className={styles.emptyRow}>{t("No servers")}</div>
          ) : (
            servers.map((server) => (
              <button
                type="button"
                className={styles.serverItem}
                key={server.id}
                onClick={() => setEditing(server)}
              >
                <span className={styles.serverItemText}>
                  <span className="server-row-name">{serverDisplayName(server)}</span>
                  <span className="server-row-url">{server.url}</span>
                </span>
                <span className="settings-row-chevron">
                  <Icon name="keyboard_arrow_right" size={14} />
                </span>
              </button>
            ))
          )}
        </div>
      </div>
      {editing !== null && (
        <ServerDialog
          server={editing === "new" ? null : editing}
          canDelete={editing !== "new" && servers.length > 0}
          onSave={saveServer}
          onDelete={deleteServer}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}

export function ServerDialog(props: {
  server: Server | null;
  canDelete: boolean;
  onSave: (server: Server) => void;
  onDelete: (id: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [name, setName] = useState(props.server?.name ?? "");
  const [url, setUrl] = useState(props.server?.url ?? "");
  const [secret, setSecret] = useState(props.server?.secret ?? "");
  const reachability = useServerReachability(url, secret);

  const normalizedUrl = normalizeServerUrl(url);
  const valid = normalizedUrl !== "";

  return (
    <Dialog onClose={props.onClose}>
      <h3>{props.server ? t("Edit Server") : t("New Server")}</h3>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!valid) {
            return;
          }
          props.onSave({
            id: props.server?.id ?? createServerId(),
            name: name.trim(),
            url: normalizedUrl,
            secret,
          });
        }}
      >
        <Field label={t("Name")}>
          <input
            className="input"
            value={name}
            placeholder={t("Optional")}
            onChange={(event) => setName(event.target.value)}
          />
        </Field>
        <Field label={t("URL")}>
          <input
            className="input"
            value={url}
            placeholder={t("Required")}
            onChange={(event) => setUrl(event.target.value)}
          />
        </Field>
        <Field label={t("Secret")}>
          <SecretInput value={secret} placeholder={t("Optional")} onChange={setSecret} />
        </Field>
        <ReachabilityIndicator reachability={reachability} url={url} />
        <div className="row-actions dialog-actions">
          {props.server && props.canDelete && (
            <Button
              variant="danger"
              style={{ marginInlineEnd: "auto" }}
              onClick={() => props.onDelete(props.server!.id)}
            >
              {t("Delete")}
            </Button>
          )}
          <Button onClick={props.onClose}>
            {t("Cancel")}
          </Button>
          <Button variant="primary" type="submit" disabled={!valid}>
            {t("Save")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
