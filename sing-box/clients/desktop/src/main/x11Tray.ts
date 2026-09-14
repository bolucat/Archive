import { nativeImage } from "electron";
import * as dbus from "dbus-next";

import { resourcePath } from "./resources";

const DBUS_SERVICE = "org.freedesktop.DBus";
const DBUS_PATH = "/org/freedesktop/DBus";
const DBUS_INTERFACE = "org.freedesktop.DBus";
const STATUS_NOTIFIER_WATCHER_SERVICE = "org.kde.StatusNotifierWatcher";
const STATUS_NOTIFIER_WATCHER_PATH = "/StatusNotifierWatcher";
const STATUS_NOTIFIER_WATCHER_INTERFACE = "org.kde.StatusNotifierWatcher";
const STATUS_NOTIFIER_ITEM_PATH = "/StatusNotifierItem";
const STATUS_NOTIFIER_ITEM_INTERFACE = "org.kde.StatusNotifierItem";

type IconPixmap = [width: number, height: number, pixels: Buffer];
type ToolTip = [iconName: string, iconPixmap: IconPixmap[], title: string, description: string];

function iconPixmaps(): IconPixmap[] {
  const image = nativeImage.createFromPath(resourcePath("tray.png"));
  return image.getScaleFactors().map((scaleFactor) => {
    const size = image.getSize(scaleFactor);
    const bitmap = image.toBitmap({ scaleFactor });
    const pixels = Buffer.alloc(bitmap.length);
    for (let offset = 0; offset < bitmap.length; offset += 4) {
      const alpha = bitmap[offset + 3];
      pixels[offset] = alpha;
      pixels[offset + 1] =
        alpha === 0 ? 0 : Math.min(255, Math.round((bitmap[offset + 2] * 255) / alpha));
      pixels[offset + 2] =
        alpha === 0 ? 0 : Math.min(255, Math.round((bitmap[offset + 1] * 255) / alpha));
      pixels[offset + 3] =
        alpha === 0 ? 0 : Math.min(255, Math.round((bitmap[offset] * 255) / alpha));
    }
    return [
      Math.round(size.width * scaleFactor),
      Math.round(size.height * scaleFactor),
      pixels,
    ];
  });
}

class StatusNotifierItem extends dbus.interface.Interface {
  readonly Category = "ApplicationStatus";
  readonly Id = "sing-box";
  readonly Title = "sing-box";
  readonly Status = "Active";
  readonly WindowId = 0;
  readonly IconName = "";
  readonly IconPixmap = iconPixmaps();
  readonly OverlayIconName = "";
  readonly OverlayIconPixmap: IconPixmap[] = [];
  readonly AttentionIconName = "";
  readonly AttentionIconPixmap: IconPixmap[] = [];
  readonly AttentionMovieName = "";
  readonly ToolTip: ToolTip = ["", [], "sing-box", ""];
  readonly IconThemePath = "";
  readonly Menu = "/NO_DBUSMENU";
  readonly ItemIsMenu = false;

  constructor(
    private readonly menu: (x: number, y: number) => void,
    private readonly primaryActivate: (x: number, y: number) => void,
  ) {
    super(STATUS_NOTIFIER_ITEM_INTERFACE);
  }

  ContextMenu(x: number, y: number) {
    this.menu(x, y);
  }

  Activate(x: number, y: number) {
    this.primaryActivate(x, y);
  }

  SecondaryActivate(x: number, y: number) {
    this.menu(x, y);
  }

  Scroll(_delta: number, _orientation: string) {}

  NewTitle() {}

  NewIcon() {}

  NewAttentionIcon() {}

  NewOverlayIcon() {}

  NewToolTip() {}

  NewStatus(status: string) {
    return status;
  }
}

StatusNotifierItem.configureMembers({
  properties: {
    Category: { signature: "s", access: dbus.interface.ACCESS_READ },
    Id: { signature: "s", access: dbus.interface.ACCESS_READ },
    Title: { signature: "s", access: dbus.interface.ACCESS_READ },
    Status: { signature: "s", access: dbus.interface.ACCESS_READ },
    WindowId: { signature: "i", access: dbus.interface.ACCESS_READ },
    IconName: { signature: "s", access: dbus.interface.ACCESS_READ },
    IconPixmap: { signature: "a(iiay)", access: dbus.interface.ACCESS_READ },
    OverlayIconName: { signature: "s", access: dbus.interface.ACCESS_READ },
    OverlayIconPixmap: { signature: "a(iiay)", access: dbus.interface.ACCESS_READ },
    AttentionIconName: { signature: "s", access: dbus.interface.ACCESS_READ },
    AttentionIconPixmap: { signature: "a(iiay)", access: dbus.interface.ACCESS_READ },
    AttentionMovieName: { signature: "s", access: dbus.interface.ACCESS_READ },
    ToolTip: { signature: "(sa(iiay)ss)", access: dbus.interface.ACCESS_READ },
    IconThemePath: { signature: "s", access: dbus.interface.ACCESS_READ },
    Menu: { signature: "o", access: dbus.interface.ACCESS_READ },
    ItemIsMenu: { signature: "b", access: dbus.interface.ACCESS_READ },
  },
  methods: {
    ContextMenu: { inSignature: "ii" },
    Activate: { inSignature: "ii" },
    SecondaryActivate: { inSignature: "ii" },
    Scroll: { inSignature: "is" },
  },
  signals: {
    NewTitle: { signature: "" },
    NewIcon: { signature: "" },
    NewAttentionIcon: { signature: "" },
    NewOverlayIcon: { signature: "" },
    NewToolTip: { signature: "" },
    NewStatus: { signature: "s" },
  },
});

interface DBusDaemon extends dbus.ClientInterface {
  on(
    event: "NameOwnerChanged",
    listener: (name: string, oldOwner: string, newOwner: string) => void,
  ): this;
}

export class X11Tray {
  readonly ready: Promise<void>;

  private readonly bus = dbus.sessionBus();
  private readonly item: StatusNotifierItem;
  private destroyed = false;

  constructor(menu: (x: number, y: number) => void, activate: (x: number, y: number) => void) {
    this.item = new StatusNotifierItem(menu, activate);
    this.bus.export(STATUS_NOTIFIER_ITEM_PATH, this.item);
    this.bus.on("error", (error: unknown) => {
      if (!this.destroyed) {
        console.error("X11 tray DBus error:", error);
      }
    });
    this.ready = this.initialize();
  }

  private async initialize() {
    const dbusObject = await this.bus.getProxyObject(DBUS_SERVICE, DBUS_PATH);
    if (this.destroyed) {
      return;
    }
    const dbusDaemon = dbusObject.getInterface<DBusDaemon>(DBUS_INTERFACE);
    dbusDaemon.on("NameOwnerChanged", (name, _oldOwner, newOwner) => {
      if (
        !this.destroyed &&
        name === STATUS_NOTIFIER_WATCHER_SERVICE &&
        newOwner !== ""
      ) {
        void this.register().catch((error: unknown) => {
          console.error("failed to re-register X11 tray:", error);
        });
      }
    });
    await this.register();
  }

  private async register() {
    const reply = await this.bus.call(
      new dbus.Message({
        destination: STATUS_NOTIFIER_WATCHER_SERVICE,
        path: STATUS_NOTIFIER_WATCHER_PATH,
        interface: STATUS_NOTIFIER_WATCHER_INTERFACE,
        member: "RegisterStatusNotifierItem",
        signature: "s",
        body: [STATUS_NOTIFIER_ITEM_PATH],
      }),
    );
    if (reply === null) {
      throw new Error("StatusNotifierWatcher returned no registration response");
    }
  }

  destroy() {
    if (this.destroyed) {
      return;
    }
    this.destroyed = true;
    this.bus.unexport(STATUS_NOTIFIER_ITEM_PATH, this.item);
    this.bus.disconnect();
  }
}
