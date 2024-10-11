import { useDebounceEffect } from "ahooks";
import { useAtomValue } from "jotai";
import { RefObject, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Virtualizer, VListHandle } from "virtua";
import ContentDisplay from "../base/content-display";
import LogItem from "./log-item";
import { atomLogLevel, atomLogList } from "./modules/store";

export const LogList = ({
  scrollRef,
}: {
  scrollRef: RefObject<HTMLElement>;
}) => {
  const { t } = useTranslation();

  const logData = useAtomValue(atomLogList);

  const virtualizerRef = useRef<VListHandle>(null);

  const shouldStickToBottom = useRef(true);

  const isFristScroll = useRef(true);

  useDebounceEffect(
    () => {
      if (shouldStickToBottom && logData.length) {
        virtualizerRef.current?.scrollToIndex(logData.length - 1, {
          align: "end",
          smooth: !isFristScroll.current,
        });

        isFristScroll.current = false;
      }
    },
    [logData],
    { wait: 100 },
  );

  const logLevel = useAtomValue(atomLogLevel);

  useEffect(() => {
    isFristScroll.current = true;
  }, [logLevel]);

  const handleRangeChange = (_start: number, end: number) => {
    if (end + 1 === logData.length) {
      shouldStickToBottom.current = true;
    } else {
      shouldStickToBottom.current = false;
    }
  };

  return logData.length ? (
    <Virtualizer
      ref={virtualizerRef}
      scrollRef={scrollRef}
      onRangeChange={handleRangeChange}
    >
      {logData.map((item, index) => {
        return <LogItem key={index} value={item} />;
      })}
    </Virtualizer>
  ) : (
    <ContentDisplay className="absolute" message={t("No Logs")} />
  );
};
