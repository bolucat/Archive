import { useAsyncEffect } from "ahooks";
import { useState } from "react";
import { formatAnsi } from "@/utils/shiki";
import { useTheme } from "@mui/material";
import { LogMessage } from "@nyanpasu/interface";
import { cn } from "@nyanpasu/ui";
import styles from "./log-item.module.scss";

export const LogItem = ({ value }: { value: LogMessage }) => {
  const { palette } = useTheme();

  const [payload, setPayload] = useState(value.payload);

  const colorMapping: { [key: string]: string } = {
    error: palette.error.main,
    warning: palette.warning.main,
    info: palette.info.main,
  };

  useAsyncEffect(async () => {
    setPayload(await formatAnsi(value.payload));
  }, [value.payload]);

  return (
    <div className="w-full select-text p-4 pb-0 pt-2 font-mono">
      <div className="flex gap-2">
        <span className="font-thin">{value.time}</span>

        <span
          className="inline-block font-semibold uppercase"
          style={{
            color: colorMapping[value.type],
          }}
        >
          {value.type}
        </span>
      </div>

      <div className="text-wrap border-b border-slate-200 pb-2">
        <p
          className={cn(
            styles.item,
            palette.mode === "dark" && styles.dark,
            "data",
          )}
          dangerouslySetInnerHTML={{
            __html: payload,
          }}
        />
      </div>
    </div>
  );
};

export default LogItem;
