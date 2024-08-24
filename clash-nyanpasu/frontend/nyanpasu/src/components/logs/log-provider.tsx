import dayjs from "dayjs";
import { useSetAtom } from "jotai";
import { useEffect } from "react";
import { atomLogData } from "@/store";
import { LogMessage, useClashWS } from "@nyanpasu/interface";

const MAX_LOG_NUM = 1000;

const time = dayjs().format("MM-DD HH:mm:ss");

export const LogProvider = () => {
  const {
    logs: { latestMessage },
  } = useClashWS();

  const setLogData = useSetAtom(atomLogData);

  useEffect(() => {
    if (!latestMessage?.data) {
      return;
    }

    const data = JSON.parse(latestMessage?.data) as LogMessage;

    setLogData((prev) => {
      if (prev.length >= MAX_LOG_NUM) {
        prev.shift();
      }

      return [...prev, { ...data, time }];
    });
  }, [latestMessage?.data, setLogData]);

  return null;
};

export default LogProvider;
