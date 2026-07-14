import { Menu, Tray, app, nativeImage } from "electron";
import type { MenuItemConstructorOptions, NativeImage, Rectangle } from "electron";

import { ServiceStatus_Type } from "../shared/gen/daemon/started_service_pb";
import { desktopLanguageFromLocale, translateDesktop } from "../shared/translations";
import type { DesktopMessageKey } from "../shared/translations";
import { managedService, startedService } from "./daemon";
import { onProfilesChanged, profilesState, selectProfile, startSelectedProfile } from "./profiles";
import { resourcePath } from "./resources";
import { daemonState } from "./state";
import { destroyTrayMenuWindow, prepareTrayMenuWindow, showTrayMenu } from "./trayMenu";

let tray: Tray | null = null;
let openWindow: () => void = () => {};

function translate(key: DesktopMessageKey): string {
  return translateDesktop(desktopLanguageFromLocale(app.getLocale()), key);
}

function ignoreErrors(promise: Promise<unknown> | undefined) {
  void promise?.catch((error) => {
    console.error("tray action:", error);
  });
}

function groupsSubmenu(): MenuItemConstructorOptions[] {
  const selectableGroups = daemonState.groups.filter((group) => group.selectable);
  const items: MenuItemConstructorOptions[] = [
    {
      label: translate("URLTest All"),
      click: () => {
        for (const group of selectableGroups) {
          ignoreErrors(startedService?.uRLTest({ outboundTag: group.tag }));
        }
      },
    },
    {
      label: translate("Close All Connections"),
      click: () => ignoreErrors(startedService?.closeAllConnections({})),
    },
    { type: "separator" },
  ];
  for (const group of selectableGroups) {
    items.push({
      label: group.tag,
      submenu: [
        {
          label: translate("URLTest"),
          click: () => ignoreErrors(startedService?.uRLTest({ outboundTag: group.tag })),
        },
        { type: "separator" },
        ...group.items.map((item) => ({
          label: item.urlTestDelay > 0 ? `${item.tag} (${item.urlTestDelay}ms)` : item.tag,
          type: "radio" as const,
          checked: item.tag === group.selected,
          click: () =>
            ignoreErrors(
              startedService?.selectOutbound({ groupTag: group.tag, outboundTag: item.tag }),
            ),
        })),
      ],
    });
  }
  return items;
}

function buildTrayTemplate(): MenuItemConstructorOptions[] {
  const started = daemonState.status === ServiceStatus_Type.STARTED;
  const { selectedId, profiles } = profilesState();
  const template: MenuItemConstructorOptions[] = [{ label: "sing-box", enabled: false }];
  if (started) {
    template.push({
      label: translate("Stop"),
      click: () => ignoreErrors(managedService?.stopService({})),
    });
  } else {
    template.push({
      label: translate("Start"),
      enabled: daemonState.connection.phase === "connected",
      click: () => ignoreErrors(startSelectedProfile()),
    });
  }
  template.push({ type: "separator" });
  if (started && daemonState.groups.some((group) => group.selectable)) {
    template.push({ label: translate("Group"), submenu: groupsSubmenu() });
  }
  template.push({
    label: translate("Profiles"),
    submenu:
      profiles.length === 0
        ? [{ label: translate("No profiles"), enabled: false }]
        : profiles.map((profile) => ({
            label: profile.name,
            type: "radio" as const,
            checked: profile.id === selectedId,
            click: () => ignoreErrors(selectProfile(profile.id)),
          })),
  });
  template.push({ type: "separator" });
  template.push({ label: translate("Open"), click: () => openWindow() });
  template.push({ label: translate("Quit"), click: () => app.quit() });
  return template;
}

export function rebuildTrayMenu() {
  // Setting a native context menu on Windows suppresses the click events used
  // by our custom tray window, so native menu rebuilding is strictly for the
  // macOS and Linux implementations.
  if (tray === null || process.platform === "win32") {
    return;
  }
  tray.setContextMenu(Menu.buildFromTemplate(buildTrayTemplate()));
}

export function initializeTray(open: () => void) {
  openWindow = open;
  if (process.platform !== "win32") {
    daemonState.on("change", rebuildTrayMenu);
    onProfilesChanged(rebuildTrayMenu);
  }
  daemonState.start();
}

export function updateTrayVisibility(enabled: boolean) {
  if (!enabled) {
    tray?.destroy();
    tray = null;
    destroyTrayMenuWindow();
    return;
  }
  if (tray !== null) {
    return;
  }
  let icon: NativeImage;
  if (process.platform === "darwin") {
    icon = nativeImage.createFromPath(resourcePath("trayTemplate.png"));
    icon.setTemplateImage(true);
  } else {
    icon = nativeImage.createFromPath(
      resourcePath(process.platform === "win32" ? "tray.ico" : "tray.png"),
    );
  }
  tray = new Tray(icon);
  tray.setToolTip("sing-box");
  // Windows delivers click and right-click as distinct events only when no
  // context menu is set — with one set it pops that menu natively instead
  // (shell/browser/ui/win/notify_icon.cc); both gestures open our own frameless
  // menu window, so no native menu is set there. macOS opens the context menu
  // on any click and never emits click once a menu is set
  // (shell/browser/ui/tray_icon_cocoa.mm), so it keeps the native menu.
  if (process.platform === "win32") {
    prepareTrayMenuWindow(tray.getBounds());
    const popMenu = (bounds: Rectangle) => {
      void showTrayMenu(bounds);
    };
    tray.on("click", (_event, bounds) => popMenu(bounds));
    tray.on("right-click", (_event, bounds) => popMenu(bounds));
    return;
  }
  rebuildTrayMenu();
}
