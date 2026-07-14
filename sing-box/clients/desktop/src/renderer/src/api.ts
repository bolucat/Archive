import type { Client, Transport } from "@connectrpc/connect";
import { createClient } from "@connectrpc/connect";

import { DebugCrashRequest_Type, ManagedService } from "@shared/gen/daemon/managed_service_pb";
import type { SystemProxyStatus } from "@shared/gen/daemon/managed_service_pb";
import { DesktopService } from "@shared/gen/experimental/boxdd/desktop_service_pb";
import type { DaemonInfo } from "@shared/gen/experimental/boxdd/desktop_service_pb";

export class DesktopApi {
  private readonly managed: Client<typeof ManagedService>;
  private readonly desktop: Client<typeof DesktopService>;

  constructor(transport: Transport) {
    this.managed = createClient(ManagedService, transport);
    this.desktop = createClient(DesktopService, transport);
  }

  async stopService(): Promise<void> {
    await this.managed.stopService({});
  }

  async systemProxyStatus(): Promise<SystemProxyStatus> {
    return this.managed.getSystemProxyStatus({});
  }

  async setSystemProxyEnabled(enabled: boolean): Promise<void> {
    await this.managed.setSystemProxyEnabled({ enabled });
  }

  async daemonInfo(): Promise<DaemonInfo> {
    return this.desktop.getDaemonInfo({});
  }

  async triggerDebugCrash(type: "go" | "native"): Promise<void> {
    await this.managed.triggerDebugCrash({
      type: type === "go" ? DebugCrashRequest_Type.GO : DebugCrashRequest_Type.NATIVE,
    });
  }

  async triggerOOMReport(): Promise<void> {
    await this.managed.triggerOOMReport({});
  }
}
