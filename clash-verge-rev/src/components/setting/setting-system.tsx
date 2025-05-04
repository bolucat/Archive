import useSWR, { mutate } from "swr";
import { useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  SettingsRounded,
  PlayArrowRounded,
  PauseRounded,
  WarningRounded,
  BuildRounded,
} from "@mui/icons-material";
import { useVerge } from "@/hooks/use-verge";
import { DialogRef, Switch } from "@/components/base";
import { SettingList, SettingItem } from "./mods/setting-comp";
import { GuardState } from "./mods/guard-state";
import { SysproxyViewer } from "./mods/sysproxy-viewer";
import { TunViewer } from "./mods/tun-viewer";
import { TooltipIcon } from "@/components/base/base-tooltip-icon";
import {
  getSystemProxy,
  getAutotemProxy,
  installService,
  getAutoLaunchStatus,
} from "@/services/cmds";
import { useLockFn } from "ahooks";
import { Button, Tooltip } from "@mui/material";
import { useSystemState } from "@/hooks/use-system-state";
import { closeAllConnections } from "@/services/api";
import { showNotice } from "@/services/noticeService";

interface Props {
  onError?: (err: Error) => void;
}

const SettingSystem = ({ onError }: Props) => {
  const { t } = useTranslation();

  const { verge, mutateVerge, patchVerge } = useVerge();

  const { data: sysproxy } = useSWR("getSystemProxy", getSystemProxy);
  const { data: autoproxy } = useSWR("getAutotemProxy", getAutotemProxy);
  const { data: autoLaunchEnabled } = useSWR(
    "getAutoLaunchStatus",
    getAutoLaunchStatus,
    { revalidateOnFocus: false }
  );

  const { isAdminMode, isSidecarMode, mutateRunningMode } = useSystemState();

  // 判断Tun模式是否可用 - 当处于服务模式或管理员模式时可用
  const isTunAvailable = !isSidecarMode || isAdminMode;

  // 当实际自启动状态与配置不同步时更新配置
  useEffect(() => {
    if (
      autoLaunchEnabled !== undefined &&
      verge &&
      verge.enable_auto_launch !== autoLaunchEnabled
    ) {
      // 静默更新配置，不触发UI刷新
      mutateVerge({ ...verge, enable_auto_launch: autoLaunchEnabled }, false);
    }
  }, [autoLaunchEnabled]);

  const sysproxyRef = useRef<DialogRef>(null);
  const tunRef = useRef<DialogRef>(null);

  const {
    enable_tun_mode,
    enable_auto_launch,
    enable_silent_start,
    enable_system_proxy,
    proxy_auto_config,
  } = verge ?? {};

  const onSwitchFormat = (_e: any, value: boolean) => value;
  const onChangeData = (patch: Partial<IVergeConfig>) => {
    mutateVerge({ ...verge, ...patch }, false);
  };

  const updateProxyStatus = async () => {
    // 等待一小段时间让系统代理状态变化
    await new Promise((resolve) => setTimeout(resolve, 100));
    await mutate("getSystemProxy");
    await mutate("getAutotemProxy");
  };

  // 安装系统服务
  const onInstallService = useLockFn(async () => {
    try {
      showNotice('info', t("Installing Service..."), 1000);
      await installService();
      showNotice('success', t("Service Installed Successfully"), 2000);
      // 重新获取运行模式
      await mutateRunningMode();
    } catch (err: any) {
      showNotice('error', err.message || err.toString(), 3000);
    }
  });

  return (
    <SettingList title={t("System Setting")}>
      <SysproxyViewer ref={sysproxyRef} />
      <TunViewer ref={tunRef} />

      <SettingItem
        label={t("Tun Mode")}
        extra={
          <>
            <TooltipIcon
              title={t("Tun Mode Info")}
              icon={SettingsRounded}
              onClick={() => tunRef.current?.open()}
            />
            {isSidecarMode && !isAdminMode && (
              <Tooltip title={t("TUN requires Service Mode")}>
                <WarningRounded sx={{ color: "warning.main", mr: 1 }} />
              </Tooltip>
            )}
            {isSidecarMode && !isAdminMode && (
              <Tooltip title={t("Install Service")}>
                <Button
                  variant="outlined"
                  color="primary"
                  size="small"
                  onClick={onInstallService}
                  sx={{ mr: 1, minWidth: "32px", p: "4px" }}
                >
                  <BuildRounded fontSize="small" />
                </Button>
              </Tooltip>
            )}
          </>
        }
      >
        <GuardState
          value={enable_tun_mode ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => {
            // 当在sidecar模式下且非管理员模式时禁用切换
            if (isSidecarMode && !isAdminMode) return;
            onChangeData({ enable_tun_mode: e });
          }}
          onGuard={(e) => {
            // 当在sidecar模式下且非管理员模式时禁用切换
            if (isSidecarMode && !isAdminMode) {
              showNotice('error', t("TUN requires Service Mode"), 2000);
              return Promise.reject(new Error(t("TUN requires Service Mode")));
            }
            return patchVerge({ enable_tun_mode: e });
          }}
        >
          <Switch edge="end" disabled={isSidecarMode && !isAdminMode} />
        </GuardState>
      </SettingItem>
      <SettingItem
        label={t("System Proxy")}
        extra={
          <>
            <TooltipIcon
              title={t("System Proxy Info")}
              icon={SettingsRounded}
              onClick={() => sysproxyRef.current?.open()}
            />
            {proxy_auto_config ? (
              autoproxy?.enable ? (
                <PlayArrowRounded sx={{ color: "success.main", mr: 1 }} />
              ) : (
                <PauseRounded sx={{ color: "error.main", mr: 1 }} />
              )
            ) : sysproxy?.enable ? (
              <PlayArrowRounded sx={{ color: "success.main", mr: 1 }} />
            ) : (
              <PauseRounded sx={{ color: "error.main", mr: 1 }} />
            )}
          </>
        }
      >
        <GuardState
          value={enable_system_proxy ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => onChangeData({ enable_system_proxy: e })}
          onGuard={async (e) => {
            if (!e && verge?.auto_close_connection) {
              closeAllConnections();
            }
            await patchVerge({ enable_system_proxy: e });
            await updateProxyStatus();
          }}
        >
          <Switch edge="end" />
        </GuardState>
      </SettingItem>

      <SettingItem 
        label={t("Auto Launch")}
        extra={
          isAdminMode && (
            <Tooltip title={t("Administrator mode may not support auto launch")}>
              <WarningRounded sx={{ color: "warning.main", mr: 1 }} />
            </Tooltip>
          )
        }
      >
        <GuardState
          value={enable_auto_launch ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => {
            // 移除管理员模式检查提示
            onChangeData({ enable_auto_launch: e });
          }}
          onGuard={async (e) => {
            if (isAdminMode) {
              showNotice('info', t("Administrator mode may not support auto launch"), 2000);
            }
            
            try {
              // 在应用更改之前先触发UI更新，让用户立即看到反馈
              onChangeData({ enable_auto_launch: e });
              await patchVerge({ enable_auto_launch: e });
              // 更新实际状态
              await mutate("getAutoLaunchStatus");
              return Promise.resolve();
            } catch (error) {
              // 如果出错，恢复原始状态
              onChangeData({ enable_auto_launch: !e });
              return Promise.reject(error);
            }
          }}
        >
          <Switch edge="end" />
        </GuardState>
      </SettingItem>

      <SettingItem
        label={t("Silent Start")}
        extra={
          <TooltipIcon title={t("Silent Start Info")} sx={{ opacity: "0.7" }} />
        }
      >
        <GuardState
          value={enable_silent_start ?? false}
          valueProps="checked"
          onCatch={onError}
          onFormat={onSwitchFormat}
          onChange={(e) => onChangeData({ enable_silent_start: e })}
          onGuard={(e) => patchVerge({ enable_silent_start: e })}
        >
          <Switch edge="end" />
        </GuardState>
      </SettingItem>
    </SettingList>
  );
};

export default SettingSystem;
