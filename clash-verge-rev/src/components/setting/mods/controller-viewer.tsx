import { BaseDialog, DialogRef } from "@/components/base";
import { useClashInfo } from "@/hooks/use-clash";
import { useVerge } from "@/hooks/use-verge";
import { showNotice } from "@/services/noticeService";
import {
  ContentCopy,
  RefreshRounded,
} from "@mui/icons-material";
import {
  Alert,
  Box,
  CircularProgress,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Snackbar,
  TextField,
  Tooltip
} from "@mui/material";
import { useLockFn } from "ahooks";
import { forwardRef, useImperativeHandle, useState } from "react";
import { useTranslation } from "react-i18next";

// 随机端口和密码生成
const generateRandomPort = (): number => {
  return Math.floor(Math.random() * (65535 - 1024 + 1)) + 1024;
};

const generateRandomPassword = (length: number = 64): string => {
  const charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let password = "";

  for (let i = 0; i < length; i++) {
    const randomIndex = Math.floor(Math.random() * charset.length);
    password += charset.charAt(randomIndex);
  }

  return password;
};

export const ControllerViewer = forwardRef<DialogRef>((props, ref) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [copySuccess, setCopySuccess] = useState<null | string>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isRestarting, setIsRestarting] = useState(false);

  const { clashInfo, patchInfo } = useClashInfo();
  const { verge, patchVerge } = useVerge();

  const [controller, setController] = useState(clashInfo?.server || "");
  const [secret, setSecret] = useState(clashInfo?.secret || "");

  const restartCoreDirectly = useLockFn(async () => {
    try {
      const controllerUrl = controller || clashInfo?.server || 'http://localhost:9090';

      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };

      if (secret) {
        headers['Authorization'] = `Bearer ${secret}`;
      }

      const response = await fetch(`${controllerUrl}/restart`, {
        method: 'POST',
        headers,
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(errorText || 'Failed to restart core');
      }

      const contentType = response.headers.get('content-type');
      if (contentType && contentType.includes('application/json')) {
        return await response.json();
      } else {
        const text = await response.text();
        console.log('Non-JSON response:', text);
        return { message: 'Restart request sent successfully' };
      }
    } catch (err: any) {
      console.error('Error restarting core:', err);
      throw err;
    }
  });

  const onSave = useLockFn(async () => {
    if (!controller.trim()) {
      showNotice('info', t("Controller address cannot be empty"), 3000);
      return;
    }

    try {
      setIsSaving(true);

      await patchInfo({ "external-controller": controller, secret });
      await patchVerge({ "external-controller": controller, secret });

      await restartCoreDirectly();

      showNotice('success', t("Configuration saved and core restarted successfully"), 2000);
      setOpen(false);
    } catch (err: any) {
      showNotice('error', err.message || t("Failed to save configuration or restart core"), 4000);
    } finally {
      setIsSaving(false);
    }
  });

  // 生成随机端口
  const handleGeneratePort = useLockFn(async () => {
    const port = generateRandomPort();
    const host = controller.split(':')[0] || '127.0.0.1';
    setController(`${host}:${port}`);
    showNotice('success', t("Random port generated"), 1000);
    return Promise.resolve();
  });

  // 生成随机 Secret
  const handleGenerateSecret = useLockFn(async () => {
    const password = generateRandomPassword();
    setSecret(password);
    showNotice('success', t("Random secret generated"), 1000);
    return Promise.resolve();
  });

  // 复制到剪贴板
  const handleCopyToClipboard = useLockFn(async (text: string, type: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopySuccess(type);
      setTimeout(() => setCopySuccess(null), 2000);
    } catch (err) {
      showNotice('error', t("Failed to copy"), 2000);
    }
  });

  // 初始化对话框
  useImperativeHandle(ref, () => ({
    open: async () => {
      setOpen(true);
      // 加载现有配置
      setController(clashInfo?.server || "");
      setSecret(clashInfo?.secret || "");
    },
    close: () => setOpen(false),
  }));

  return (
    <BaseDialog
      open={open}
      title={t("External Controller")}
      contentSx={{ width: 400 }}
      okBtn={
        isSaving ? (
          <Box display="flex" alignItems="center" gap={1}>
            <CircularProgress size={16} color="inherit" />
            {t("Saving...")}
          </Box>
        ) : (
          t("Save")
        )
      }
      cancelBtn={t("Cancel")}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
      onOk={onSave}
    >
      <List>
        <ListItem sx={{ padding: "5px 2px", display: "flex", justifyContent: "space-between" }}>
          <Box display="flex" alignItems="center" gap={1}>
            <ListItemText primary={t("External Controller")} />
            <Tooltip title={t("Generate Random Port")}>
              <IconButton
                size="small"
                onClick={handleGeneratePort}
                color="primary"
                disabled={isSaving || isRestarting}
              >
                <RefreshRounded fontSize="small" />
              </IconButton>
            </Tooltip>
          </Box>
          <Box display="flex" alignItems="center" gap={1}>
            <TextField
              autoComplete="new-password"
              size="small"
              sx={{ width: 175 }}
              value={controller}
              placeholder="Required"
              onChange={(e) => setController(e.target.value)}
              disabled={isSaving || isRestarting}
            />
            <Tooltip title={t("Copy to clipboard")}>
              <IconButton
                size="small"
                onClick={() => handleCopyToClipboard(controller, "controller")}
                color="primary"
                disabled={isSaving || isRestarting}
              >
                <ContentCopy fontSize="small" />
              </IconButton>
            </Tooltip>
          </Box>
        </ListItem>

        <ListItem sx={{ padding: "5px 2px", display: "flex", justifyContent: "space-between" }}>
          <Box display="flex" alignItems="center" gap={1}>
            <ListItemText primary={t("Core Secret")} />
            <Tooltip title={t("Generate Random Secret")}>
              <IconButton
                size="small"
                onClick={handleGenerateSecret}
                color="primary"
                disabled={isSaving || isRestarting}
              >
                <RefreshRounded fontSize="small" />
              </IconButton>
            </Tooltip>
          </Box>
          <Box display="flex" alignItems="center" gap={1}>
            <TextField
              autoComplete="new-password"
              size="small"
              sx={{ width: 175 }}
              value={secret}
              placeholder={t("Recommended")}
              onChange={(e) =>
                setSecret(e.target.value?.replace(/[^\x00-\x7F]/g, ""))
              }
              disabled={isSaving || isRestarting}
            />
            <Tooltip title={t("Copy to clipboard")}>
              <IconButton
                size="small"
                onClick={() => handleCopyToClipboard(secret, "secret")}
                color="primary"
                disabled={isSaving || isRestarting}
              >
                <ContentCopy fontSize="small" />
              </IconButton>
            </Tooltip>
          </Box>
        </ListItem>
      </List>

      <Snackbar
        open={copySuccess !== null}
        autoHideDuration={2000}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
      >
        <Alert
          severity="success"
          sx={{ width: '100%' }}
        >
          {copySuccess === "controller"
            ? t("Controller address copied to clipboard")
            : t("Secret copied to clipboard")
          }
        </Alert>
      </Snackbar>
    </BaseDialog>
  );
});
