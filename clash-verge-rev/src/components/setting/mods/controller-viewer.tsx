import { forwardRef, useImperativeHandle, useState, useEffect } from "react";
import { useLockFn } from "ahooks";
import { useTranslation } from "react-i18next";
import { List, ListItem, ListItemText, TextField, Typography, Box } from "@mui/material";
import { useClashInfo } from "@/hooks/use-clash";
import { BaseDialog, DialogRef, Notice } from "@/components/base";
import { useVerge } from "@/hooks/use-verge";
import { useClash } from "@/hooks/use-clash";

export const ControllerViewer = forwardRef<DialogRef>((props, ref) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const { clashInfo, patchInfo } = useClashInfo();
  const { verge, patchVerge } = useVerge();
  const { clash } = useClash();

  const [controller, setController] = useState("");
  const [secret, setSecret] = useState("");
  
  const enableController = Boolean(clash?.["external-controller"] && clash?.["external-controller"] !== "");

  useImperativeHandle(ref, () => ({
    open: () => {
      setOpen(true);
      setController(clash?.["external-controller"] || "");
      setSecret(clash?.secret || "");
    },
    close: () => setOpen(false),
  }));

  const onSave = useLockFn(async () => {
    try {
      setOpen(false);
      const promises = [];
      promises.push(
        patchInfo({ 
          "external-controller": controller || "127.0.0.1:9097", 
          secret 
        })
      );
      
      // 同步verge配置
      if (controller && controller !== "") {
        promises.push(patchVerge({ enable_external_controller: true }));
      }
      await Promise.all(promises);
      Notice.success(t("External Controller Settings Saved"), 1000);
    } catch (err: any) {
      Notice.error(err.message || err.toString(), 4000);
    }
  });

  return (
    <BaseDialog
      open={open}
      title={t("External Controller")}
      contentSx={{ width: 400 }}
      okBtn={t("Save")}
      cancelBtn={t("Cancel")}
      disableOk={!enableController}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
      onOk={onSave}
    >
      <Box>
        <Typography variant="body2" color={enableController ? "warning.main" : "text.secondary"}>
          {enableController 
            ? t("External controller is enabled info") 
            : t("External controller is disabled info")}
        </Typography>
      </Box>
      
      <List>
        <ListItem sx={{ padding: "5px 2px" }}>
          <ListItemText primary={t("External Controller Address")} />
          <TextField
            autoComplete="new-password"
            size="small"
            sx={{ width: 175 }}
            value={controller}
            placeholder="127.0.0.1:9097"
            onChange={(e) => setController(e.target.value)}
            disabled={!enableController}
          />
        </ListItem>

        <ListItem sx={{ padding: "5px 2px" }}>
          <ListItemText primary={t("Core Secret")} />
          <TextField
            autoComplete="new-password"
            size="small"
            sx={{ width: 175 }}
            value={secret}
            placeholder={t("Recommended")}
            onChange={(e) =>
              setSecret(e.target.value?.replace(/[^\x00-\x7F]/g, ""))
            }
            disabled={!enableController}
          />
        </ListItem>
      </List>
    </BaseDialog>
  );
});
