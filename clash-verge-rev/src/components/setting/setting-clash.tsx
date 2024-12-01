import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { TextField, Select, MenuItem, Typography } from "@mui/material";
import {
  SettingsRounded,
  ShuffleRounded,
  LanRounded,
} from "@mui/icons-material";
import { DialogRef, Notice, Switch } from "@/components/base";
import { useClash } from "@/hooks/use-clash";
import { GuardState } from "./mods/guard-state";
import { WebUIViewer } from "./mods/web-ui-viewer";
import { ClashPortViewer } from "./mods/clash-port-viewer";
import { ControllerViewer } from "./mods/controller-viewer";
import { SettingList, SettingItem } from "./mods/setting-comp";
import { ClashCoreViewer } from "./mods/clash-core-viewer";
import { invoke_uwp_tool } from "@/services/cmds";
import getSystem from "@/utils/get-system";
import { useVerge } from "@/hooks/use-verge";
import { updateGeoData } from "@/services/api";
import { TooltipIcon } from "@/components/base/base-tooltip-icon";
import { NetworkInterfaceViewer } from "./mods/network-interface-viewer";

const isWIN = getSystem() === "windows";

interface Props {
  onError: (err: Error) => void;
}

const SettingClash = ({ onError }: Props) => {
  const { t } = useTranslation();

  const { clash, version, mutateClash, patchClash } = useClash();
  const { verge, mutateVerge, patchVerge } = useVerge();

  const {
    ipv6,
    "allow-lan": allowLan,
    "log-level": logLevel,
    "unified-delay": unifiedDelay,
  } = clash ?? {};

  const { enable_random_port = false, verge_mixed_port } = verge ?? {};

  const webRef = useRef<DialogRef>(null);
  const portRef = useRef<DialogRef>(null);
  const ctrlRef = useRef<DialogRef>(null);
  const coreRef = useRef<DialogRef>(null);
  const networkRef = useRef<DialogRef>(null);

  const onSwitchFormat = (_e: any, value: boolean) => value;
  const onChangeData = (patch: Partial<IConfigData>) => {
    mutateClash((old) => ({ ...(old! || {}), ...patch }), false);
  };
  const onChangeVerge = (patch: Partial<IVergeConfig>) => {
    mutateVerge({ ...verge, ...patch }, false);
  };
  const onUpdateGeo = async () => {
    try {
      await updateGeoData();
      Notice.success(t("GeoData Updated"));
    } catch (err: any) {
      Notice.error(err?.response.data.message || err.toString());
    }
  };

  return (
    <SettingList title={t("Clash Setting")}>
      <WebUIViewer ref={webRef} />
      <ClashPortViewer ref={portRef} />
      <ControllerViewer ref={ctrlRef} />
      <ClashCoreViewer ref={coreRef} />
      <NetworkInterfaceViewer ref={networkRef} />

      <SettingItem
        label={t("Allow Lan")}
        extra={
          <TooltipIcon
            title={t("Network Interface")}
            color={"inherit"}
            icon={LanRounded}
            onClick={() => {
              networkRef.current?.open();
            }}
          />
        }
      >
        <GuardState
          value={allowLan ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => onChangeData({ "allow-lan": e })}
          onGuard={(e) => patchClash({ "allow-lan": e })}
        >
          <Switch edge="end" />
        </GuardState>
      </SettingItem>

      <SettingItem label={t("IPv6")}>
        <GuardState
          value={ipv6 ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => onChangeData({ ipv6: e })}
          onGuard={(e) => patchClash({ ipv6: e })}
        >
          <Switch edge="end" />
        </GuardState>
      </SettingItem>

      <SettingItem
        label={t("Unified Delay")}
        extra={
          <TooltipIcon
            title={t("Unified Delay Info")}
            sx={{ opacity: "0.7" }}
          />
        }
      >
        <GuardState
          value={unifiedDelay ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => onChangeData({ "unified-delay": e })}
          onGuard={(e) => patchClash({ "unified-delay": e })}
        >
          <Switch edge="end" />
        </GuardState>
      </SettingItem>

      <SettingItem
        label={t("Log Level")}
        extra={
          <TooltipIcon title={t("Log Level Info")} sx={{ opacity: "0.7" }} />
        }
      >
        <GuardState
          // clash premium 2022.08.26 值为warn
          value={logLevel === "warn" ? "warning" : (logLevel ?? "info")}
          onCatch={onError}
          onFormat={(e: any) => e.target.value}
          onChange={(e) => onChangeData({ "log-level": e })}
          onGuard={(e) => patchClash({ "log-level": e })}
        >
          <Select size="small" sx={{ width: 100, "> div": { py: "7.5px" } }}>
            <MenuItem value="debug">Debug</MenuItem>
            <MenuItem value="info">Info</MenuItem>
            <MenuItem value="warning">Warn</MenuItem>
            <MenuItem value="error">Error</MenuItem>
            <MenuItem value="silent">Silent</MenuItem>
          </Select>
        </GuardState>
      </SettingItem>

      <SettingItem
        label={t("Port Config")}
        extra={
          <TooltipIcon
            title={t("Random Port")}
            color={enable_random_port ? "primary" : "inherit"}
            icon={ShuffleRounded}
            onClick={() => {
              Notice.success(
                t("Restart Application to Apply Modifications"),
                1000,
              );
              onChangeVerge({ enable_random_port: !enable_random_port });
              patchVerge({ enable_random_port: !enable_random_port });
            }}
          />
        }
      >
        <TextField
          autoComplete="new-password"
          disabled={enable_random_port}
          size="small"
          value={verge_mixed_port ?? 7897}
          sx={{ width: 100, input: { py: "7.5px", cursor: "pointer" } }}
          onClick={(e) => {
            portRef.current?.open();
            (e.target as any).blur();
          }}
        />
      </SettingItem>

      <SettingItem
        onClick={() => ctrlRef.current?.open()}
        label={t("External")}
      />

      <SettingItem onClick={() => webRef.current?.open()} label={t("Web UI")} />

      <SettingItem
        label={t("Clash Core")}
        extra={
          <TooltipIcon
            icon={SettingsRounded}
            onClick={() => coreRef.current?.open()}
          />
        }
      >
        <Typography sx={{ py: "7px", pr: 1 }}>{version}</Typography>
      </SettingItem>

      {isWIN && (
        <SettingItem
          onClick={invoke_uwp_tool}
          label={t("Open UWP tool")}
          extra={
            <TooltipIcon
              title={t("Open UWP tool Info")}
              sx={{ opacity: "0.7" }}
            />
          }
        />
      )}

      <SettingItem onClick={onUpdateGeo} label={t("Update GeoData")} />
    </SettingList>
  );
};

export default SettingClash;
