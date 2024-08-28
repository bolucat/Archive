import { useState } from "react";
import getSystem from "@/utils/get-system";
import LoadingButton from "@mui/lab/LoadingButton";
import { open } from "@tauri-apps/api/dialog";
import { readTextFile } from "@tauri-apps/api/fs";

const isWin = getSystem() === "windows";

export interface ReadProfileProps {
  onSelected: (content: string) => void;
}

export const ReadProfile = ({ onSelected }: ReadProfileProps) => {
  const [loading, setLoading] = useState(false);

  const [label, setLabel] = useState("");

  const handleSelectFile = async () => {
    try {
      setLoading(true);

      const selected = await open({
        directory: false,
        multiple: false,
        filters: [
          {
            name: "profile",
            extensions: ["yaml", "yml"],
          },
        ],
      });

      // user cancelled the selection
      if (!selected || Array.isArray(selected)) {
        return null;
      }

      onSelected(await readTextFile(selected));

      if (isWin) {
        setLabel(selected.split("\\").at(-1) as string);
      } else {
        setLabel(selected.split("/").at(-1) as string);
      }
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <LoadingButton
      variant="contained"
      loading={loading}
      disabled={loading}
      onClick={handleSelectFile}
      color={label ? "success" : "primary"}
    >
      {label || "Select File"}
    </LoadingButton>
  );
};
