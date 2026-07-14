import { useEffect, useMemo, useState } from "react";
import { appendFileHeaderMetaToBuffer, blockToBinary, createEncoder } from "luby-transform";

import type { DesktopHost, DesktopProfile } from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { Button, Dialog, QRCode, Spinner } from "../components/ui";
import styles from "./ProfileQRSDialog.module.css";

const QRS_URL_PREFIX = "https://qrss.netlify.app/#";
const DEFAULT_FPS = 10;
const DEFAULT_SLICE_SIZE = 500;

function SliderField(props: {
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className={styles.slider}>
      <div className={styles.sliderHeader}>
        <span>{props.label}</span>
        <span className={styles.sliderValue}>{props.value}</span>
      </div>
      <input
        className={styles.sliderInput}
        type="range"
        min={props.min}
        max={props.max}
        step={props.step}
        value={props.value}
        aria-label={props.label}
        onChange={(event) => props.onChange(Number(event.target.value))}
      />
    </div>
  );
}

export function ProfileQRSDialog(props: {
  host: DesktopHost;
  profile: DesktopProfile;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [payload, setPayload] = useState<Uint8Array | null>(null);
  const [fps, setFps] = useState(DEFAULT_FPS);
  const [sliceSize, setSliceSize] = useState(DEFAULT_SLICE_SIZE);
  const [frame, setFrame] = useState<string | null>(null);

  useEffect(() => {
    let stale = false;
    props.host.profiles
      .encodeData(props.profile.id)
      .then((data) => {
        if (!stale) {
          setPayload(
            appendFileHeaderMetaToBuffer(data, {
              filename: `${props.profile.name}.bpf`,
              contentType: "application/octet-stream",
            }),
          );
        }
      })
      .catch((error: unknown) => {
        showError(error);
        props.onClose();
      });
    return () => {
      stale = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.profile.id]);

  const fountain = useMemo(
    () => (payload === null ? null : createEncoder(payload, sliceSize).fountain()),
    [payload, sliceSize],
  );

  useEffect(() => {
    if (fountain === null) {
      return;
    }
    const nextFrame = () => {
      let raw = "";
      for (const byte of blockToBinary(fountain.next().value)) {
        raw += String.fromCharCode(byte);
      }
      setFrame(QRS_URL_PREFIX + btoa(raw));
    };
    nextFrame();
    const timer = window.setInterval(nextFrame, 1000 / fps);
    return () => window.clearInterval(timer);
  }, [fountain, fps]);

  return (
    <Dialog onClose={props.onClose}>
      <h3>{t("Share as QRS Code")}</h3>
      {frame !== null ? (
        <QRCode value={frame} />
      ) : (
        <div className={styles.placeholder}>
          <Spinner />
        </div>
      )}
      <SliderField label={t("FPS")} min={1} max={60} step={1} value={fps} onChange={setFps} />
      <SliderField
        label={t("Slice Size")}
        min={100}
        max={1500}
        step={100}
        value={sliceSize}
        onChange={setSliceSize}
      />
      <div className="row-actions dialog-actions">
        <Button href="https://github.com/qifi-dev/qrs" target="_blank" rel="noreferrer">
          {t("What is QRS")}
        </Button>
        <Button variant="primary" onClick={props.onClose}>
          {t("Close")}
        </Button>
      </div>
    </Dialog>
  );
}
