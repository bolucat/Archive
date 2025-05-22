import useSWR, { mutate } from "swr";
import { useLockFn } from "ahooks";
import { getVergeConfig, patchVergeConfig } from "@/services/cmds";

interface IVergeConfigExtended extends IVergeConfig {
  "port"?: number;
  "socks-port"?: number;
  "mixed-port"?: number;
  "redir-port"?: number;
  "tproxy-port"?: number;
  "api-port"?: number;
  "secret"?: string;
  "external-controller"?: string; // 添加 missing 属性
}

export const useVerge = () => {
  const { data: verge, mutate: mutateVerge } = useSWR(
    "getVergeConfig",
    async () => {
      const config = await getVergeConfig();
      return config;
    },
  );

  const patchVerge = useLockFn(async (patch: Partial<IVergeConfigExtended>) => {
    const hasInfo =
      patch["redir-port"] != null ||
      patch["tproxy-port"] != null ||
      patch["mixed-port"] != null ||
      patch["socks-port"] != null ||
      patch["port"] != null ||
      patch["external-controller"] != null ||
      patch.secret != null;

    if (!hasInfo) return;

    // 端口验证逻辑
    const validatePort = (portName: string, portValue: number) => {
      if (portValue < 1000) {
        throw new Error(`The ${portName} should not < 1000`);
      }
      if (portValue > 65535) {
        throw new Error(`The ${portName} should not > 65535`);
      }
    };

    if (patch["port"]) validatePort("port", patch["port"]);
    if (patch["socks-port"]) validatePort("socks-port", patch["socks-port"]);
    if (patch["mixed-port"]) validatePort("mixed-port", patch["mixed-port"]);
    if (patch["redir-port"]) validatePort("redir-port", patch["redir-port"]);
    if (patch["tproxy-port"]) validatePort("tproxy-port", patch["tproxy-port"]);
    if (patch["api-port"]) validatePort("api-port", patch["api-port"]);

    await patchVergeConfig(patch);
    mutateVerge();
    mutate("getVergeConfig");
  });

  return {
    verge,
    mutateVerge,
    patchVerge,
  };
};
