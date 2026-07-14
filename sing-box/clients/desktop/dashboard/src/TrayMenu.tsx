import { useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent } from "react";

import { DaemonApi } from "./api/daemon";
import type { Server } from "./api/config";
import { urlTestDelayTone } from "./api/format";
import { useStream } from "./api/stream";
import {
  applyAccent,
  applyTheme,
  loadAccentPreference,
  loadThemePreference,
  watchSystemTheme,
} from "./app/context";
import { useDaemonConnection, useDesktopProfiles } from "./app/desktop";
import type { DesktopHost } from "./app/desktop";
import { dismissError, showError, useCurrentError } from "./app/errorStore";
import { I18nProvider, useI18n } from "./app/i18n";
import { Icon } from "./components/Icon";
import { Spinner, Switch } from "./components/ui";
import { ServiceStatus_Type } from "./gen/daemon/started_service_pb";
import type { Group } from "./gen/daemon/started_service_pb";
import { cx } from "./lib/cx";
import { watchStoredValues } from "./lib/storage";
import styles from "./TrayMenu.module.css";

const TRAY_LOCAL_SERVER: Server = { id: "tray-local", name: "sing-box", url: "", secret: "" };

const HOVER_CLOSE_DELAY = 180;

const GROUPS_SUBMENU = "\0groups";
const PROFILES_SUBMENU = "\0profiles";

export function TrayMenu(props: { desktop: DesktopHost }) {
  return (
    <I18nProvider>
      <TrayMenuContent host={props.desktop} />
    </I18nProvider>
  );
}

function useTrayTheme() {
  useEffect(() => {
    const apply = () => {
      applyTheme(loadThemePreference());
      applyAccent(loadAccentPreference());
    };
    apply();
    const unwatchSystem = watchSystemTheme(loadThemePreference);
    const unwatchStored = watchStoredValues(apply);
    return () => {
      unwatchSystem();
      unwatchStored();
    };
  }, []);
}

interface SubmenuController {
  activeKey: string | null;
  open: (key: string) => void;
  scheduleClose: () => void;
  cancelClose: () => void;
  close: () => void;
}

function useSubmenuController(): SubmenuController {
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const closeTimer = useRef<number | null>(null);

  const cancelClose = () => {
    if (closeTimer.current !== null) {
      clearTimeout(closeTimer.current);
      closeTimer.current = null;
    }
  };
  const open = (key: string) => {
    cancelClose();
    setActiveKey(() => key);
  };
  const close = () => {
    cancelClose();
    setActiveKey(null);
  };
  const scheduleClose = () => {
    cancelClose();
    closeTimer.current = window.setTimeout(close, HOVER_CLOSE_DELAY);
  };
  useEffect(() => cancelClose, []);
  return { activeKey, open, scheduleClose, cancelClose, close };
}

function useMenuKeyboard(controller: SubmenuController, closeMenu: () => void) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (controller.activeKey !== null) {
          controller.close();
        } else {
          closeMenu();
        }
        return;
      }
      if (event.key === "ArrowLeft" && controller.activeKey !== null) {
        controller.close();
        return;
      }
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
        return;
      }
      const focusables = [...document.querySelectorAll<HTMLElement>("button:not(:disabled)")];
      if (focusables.length === 0) {
        return;
      }
      const index = focusables.indexOf(document.activeElement as HTMLElement);
      const step = event.key === "ArrowDown" ? 1 : -1;
      focusables[(index + step + focusables.length) % focusables.length].focus();
      event.preventDefault();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [controller, closeMenu]);
}

function TrayMenuContent(props: { host: DesktopHost }) {
  const host = props.host;
  const { t } = useI18n();
  const closeMenu = () => host.application.closeTrayMenu();
  useTrayTheme();
  const controller = useSubmenuController();
  useMenuKeyboard(controller, closeMenu);
  const api = useMemo(() => new DaemonApi(TRAY_LOCAL_SERVER, host.transport), [host]);
  const connection = useDaemonConnection(host);
  const serviceStatus = useStream(api.serviceStatus);
  const groups = useStream(api.groups);
  const { selectedId, profiles } = useDesktopProfiles(host);
  const errorMessage = useCurrentError();
  const [busy, setBusy] = useState(false);

  const connected = connection.phase === "connected";
  const statusType = serviceStatus.data.status?.status ?? ServiceStatus_Type.IDLE;
  const started = statusType === ServiceStatus_Type.STARTED;
  const switchOn =
    started ||
    statusType === ServiceStatus_Type.STARTING ||
    statusType === ServiceStatus_Type.STOPPING;
  const transitioning =
    statusType === ServiceStatus_Type.STARTING || statusType === ServiceStatus_Type.STOPPING;

  useEffect(() => {
    if (started) {
      api.groups.retryNow();
    }
  }, [started, api]);

  const toggleService = (value: boolean) => {
    setBusy(true);
    (value ? host.service.start() : host.service.stop())
      .catch(showError)
      .finally(() => {
        api.serviceStatus.reconnectNow();
        setBusy(false);
      });
  };

  const selectableGroups = groups.data.groups.filter((group) => group.selectable);
  const running = connected && started;

  const dismissOnBackground = (event: MouseEvent) => {
    if (event.target === event.currentTarget) {
      closeMenu();
    }
  };

  const cascade =
    controller.activeKey === null ? null : (
      controller.activeKey === PROFILES_SUBMENU ? (
        <div
          className={styles.submenu}
          onMouseEnter={controller.cancelClose}
          onMouseLeave={controller.scheduleClose}
        >
          <ProfilesSubmenu
            profiles={profiles}
            selectedId={selectedId}
            onSelect={(id) => host.profiles.select(id).then(closeMenu).catch(showError)}
          />
        </div>
      ) : (
        <GroupsCascade
          groups={selectableGroups}
          api={api}
          onMouseEnter={controller.cancelClose}
          onMouseLeave={controller.scheduleClose}
        />
      )
    );

  return (
    <div
      className={styles.viewport}
      role="presentation"
      onMouseLeave={controller.scheduleClose}
      onMouseDown={dismissOnBackground}
    >
      <div
        className={styles.content}
        role="presentation"
        data-tray-content
        onMouseDown={dismissOnBackground}
      >
        <div className={styles.panel} data-tray-panel>
          <div className={styles.header}>
            <span className={styles.title}>sing-box</span>
            {connected ? (
              <Switch
                label={t("Service")}
                value={switchOn}
                disabled={busy || transitioning || (!switchOn && profiles.length === 0)}
                onChange={toggleService}
              />
            ) : (
              <Spinner />
            )}
          </div>
          {errorMessage !== null && (
            <div className={styles.error}>
              <span className={styles.errorText}>{errorMessage}</span>
              <button type="button" className={styles.errorDismiss} aria-label={t("Ok")} onClick={dismissError}>
                <Icon name="close" size={14} />
              </button>
            </div>
          )}
          {running && selectableGroups.length > 0 && (
            <div className={styles.section}>
              <ParentRow
                menuKey={GROUPS_SUBMENU}
                label={t("Groups")}
                detail=""
                controller={controller}
              />
            </div>
          )}
          <div className={styles.section}>
            <ParentRow
              menuKey={PROFILES_SUBMENU}
              label={t("Profiles")}
              detail={profiles.find((profile) => profile.id === selectedId)?.name ?? ""}
              controller={controller}
            />
          </div>
          <div className={styles.section}>
            <button
              type="button"
              className={styles.row}
              onMouseEnter={controller.close}
              onClick={() => {
                host.application.showMainWindow();
                closeMenu();
              }}
            >
              <span className={styles.rowIcon}>
                <Icon name="open_in_new" size={16} />
              </span>
              <span className={styles.rowLabel}>{t("Open")}</span>
            </button>
            <button
              type="button"
              className={styles.row}
              onMouseEnter={controller.close}
              onClick={host.application.quit}
            >
              <span className={styles.rowIcon}>
                <Icon name="power_settings_new" size={16} />
              </span>
              <span className={styles.rowLabel}>{t("Quit")}</span>
            </button>
          </div>
        </div>
        {cascade}
      </div>
    </div>
  );
}

function ParentRow(props: {
  menuKey: string;
  label: string;
  detail: string;
  controller: SubmenuController;
}) {
  const { menuKey, label, detail, controller } = props;
  const active = controller.activeKey === menuKey;
  return (
    <button
      type="button"
      className={cx(styles.row, active && styles.rowActive)}
      aria-haspopup="menu"
      aria-expanded={active}
      onMouseEnter={() => controller.open(menuKey)}
      onMouseLeave={controller.scheduleClose}
      onClick={() => controller.open(menuKey)}
      onKeyDown={(event) => {
        if (event.key === "ArrowRight" || event.key === "Enter") {
          event.preventDefault();
          controller.open(menuKey);
        }
      }}
    >
      <span className={styles.rowLabel}>{label}</span>
      {detail !== "" && <span className={styles.rowDetail}>{detail}</span>}
      <Icon name="keyboard_arrow_right" size={16} />
    </button>
  );
}

function GroupsCascade(props: {
  groups: Group[];
  api: DaemonApi;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
}) {
  const [activeTag, setActiveTag] = useState<string | null>(null);
  const closeTimer = useRef<number | null>(null);
  const activeGroup = props.groups.find((group) => group.tag === activeTag) ?? null;
  const largestGroup = props.groups.reduce<Group | null>(
    (largest, group) => largest === null || group.items.length > largest.items.length ? group : largest,
    null,
  );

  const cancelClose = () => {
    if (closeTimer.current !== null) {
      clearTimeout(closeTimer.current);
      closeTimer.current = null;
    }
  };
  const close = () => {
    cancelClose();
    setActiveTag(null);
  };
  const scheduleClose = () => {
    cancelClose();
    closeTimer.current = window.setTimeout(close, HOVER_CLOSE_DELAY);
  };
  const open = (tag: string) => {
    cancelClose();
    setActiveTag(() => tag);
  };

  useEffect(() => cancelClose, []);
  return (
    <div
      className={styles.cascade}
      onMouseEnter={props.onMouseEnter}
      onMouseLeave={props.onMouseLeave}
    >
      <div className={styles.submenu}>
        <GroupsSubmenu
          groups={props.groups}
          api={props.api}
          activeTag={activeTag}
          onOpen={open}
          onClose={close}
          onScheduleClose={scheduleClose}
        />
      </div>
      <div
        className={cx(styles.nodeSlot, activeGroup === null && styles.nodeSlotInactive)}
        onMouseEnter={cancelClose}
        onMouseLeave={scheduleClose}
      >
        {largestGroup !== null && <GroupNodesPlaceholder group={largestGroup} />}
        {activeGroup !== null && (
          <div className={styles.submenu}>
            <GroupNodes group={activeGroup} api={props.api} onClose={close} />
          </div>
        )}
      </div>
    </div>
  );
}

function GroupsSubmenu(props: {
  groups: Group[];
  api: DaemonApi;
  activeTag: string | null;
  onOpen: (tag: string) => void;
  onClose: () => void;
  onScheduleClose: () => void;
}) {
  const { t } = useI18n();
  const [testingAll, setTestingAll] = useState(false);

  return (
    <>
      <button
        type="button"
        className={styles.row}
        disabled={testingAll}
        onMouseEnter={props.onClose}
        onClick={() => {
          setTestingAll(true);
          Promise.all(props.groups.map((group) => props.api.urlTest(group.tag)))
            .catch(showError)
            .finally(() => setTestingAll(false));
        }}
      >
        <span className={styles.rowIcon}>
          {testingAll ? <Spinner /> : <Icon name="speed" size={16} />}
        </span>
        <span className={styles.rowLabel}>{t("URLTest All")}</span>
      </button>
      <button
        type="button"
        className={styles.row}
        onMouseEnter={props.onClose}
        onClick={() => props.api.closeAllConnections().catch(showError)}
      >
        <span className={styles.rowIcon}>
          <Icon name="close" size={16} />
        </span>
        <span className={styles.rowLabel}>{t("Close All Connections")}</span>
      </button>
      <div className={styles.section}>
        {props.groups.map((group) => (
          <button
            type="button"
            key={group.tag}
            className={cx(styles.row, props.activeTag === group.tag && styles.rowActive)}
            aria-haspopup="menu"
            aria-expanded={props.activeTag === group.tag}
            onMouseEnter={() => props.onOpen(group.tag)}
            onMouseLeave={props.onScheduleClose}
            onClick={() => props.onOpen(group.tag)}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight" || event.key === "Enter") {
                event.preventDefault();
                props.onOpen(group.tag);
              }
            }}
          >
            <span className={styles.rowLabel}>{group.tag}</span>
            {group.selected !== "" && <span className={styles.rowDetail}>{group.selected}</span>}
            <Icon name="keyboard_arrow_right" size={16} />
          </button>
        ))}
      </div>
    </>
  );
}

function GroupNodes(props: { group: Group; api: DaemonApi; onClose: () => void }) {
  const { t } = useI18n();
  const group = props.group;
  const [testing, setTesting] = useState(false);

  const runURLTest = () => {
    setTesting(true);
    props.api
      .urlTest(group.tag)
      .catch(showError)
      .finally(() => setTesting(false));
  };

  const selectItem = (tag: string) => {
    if (tag === group.selected) {
      return;
    }
    props.api.selectOutbound(group.tag, tag).catch(showError);
  };

  return (
    <>
      <button
        type="button"
        className={styles.row}
        disabled={testing}
        onClick={runURLTest}
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft") {
            event.preventDefault();
            event.stopPropagation();
            props.onClose();
          }
        }}
      >
        <span className={styles.rowIcon}>
          {testing ? <Spinner /> : <Icon name="speed" size={16} />}
        </span>
        <span className={styles.rowLabel}>{t("URLTest")}</span>
      </button>
      <div className={styles.submenuList}>
        {group.items.map((item) => (
          <button
            type="button"
            key={item.tag}
            className={styles.row}
            onClick={() => selectItem(item.tag)}
            onKeyDown={(event) => {
              if (event.key === "ArrowLeft") {
                event.preventDefault();
                event.stopPropagation();
                props.onClose();
              }
            }}
          >
            <span className={styles.rowIcon}>
              {item.tag === group.selected && <Icon name="check" size={16} />}
            </span>
            <span className={styles.rowLabel}>{item.tag}</span>
            {item.urlTestDelay > 0 && (
              <span className={cx(styles.delay, styles[urlTestDelayTone(item.urlTestDelay)])}>
                {item.urlTestDelay}ms
              </span>
            )}
          </button>
        ))}
      </div>
    </>
  );
}

function GroupNodesPlaceholder(props: { group: Group }) {
  return (
    <div className={cx(styles.submenu, styles.submenuPlaceholder)} aria-hidden="true">
      <div className={styles.row}>
        <span className={styles.rowIcon} />
        <span className={styles.rowLabel}>URLTest</span>
      </div>
      <div className={styles.submenuList}>
        {props.group.items.map((item) => (
          <div key={item.tag} className={styles.row}>
            <span className={styles.rowIcon} />
            <span className={styles.rowLabel}>{item.tag}</span>
            {item.urlTestDelay > 0 && <span className={styles.delay}>{item.urlTestDelay}ms</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

function ProfilesSubmenu(props: {
  profiles: { id: string; name: string }[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const { t } = useI18n();
  if (props.profiles.length === 0) {
    return <div className={styles.emptyRow}>{t("Empty profiles")}</div>;
  }
  return (
    <div className={styles.submenuList}>
      {props.profiles.map((profile) => (
        <button type="button" key={profile.id} className={styles.row} onClick={() => props.onSelect(profile.id)}>
          <span className={styles.rowIcon}>
            {profile.id === props.selectedId && <Icon name="check" size={16} />}
          </span>
          <span className={styles.rowLabel}>{profile.name}</span>
        </button>
      ))}
    </div>
  );
}
