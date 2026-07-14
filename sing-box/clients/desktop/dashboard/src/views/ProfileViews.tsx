import { lazy, Suspense, useEffect, useRef, useState } from "react";

import { formatRelativeTime } from "../api/format";
import { describeError } from "../api/stream";
import { navigate, useNow } from "../app/context";
import type { DesktopHost, DesktopProfile, DesktopProfileType } from "../app/desktop";
import { useDesktopProfiles } from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import type { JsonEditorHandle } from "../components/JsonEditor";
import {
  Button,
  Card,
  Dialog,
  Field,
  IconButton,
  MenuItem,
  OthersMenu,
  QRCode,
  Select,
  SubMenu,
  Switch,
  Toggle,
} from "../components/ui";
import { ProfileQRSDialog } from "./ProfileQRSDialog";
import styles from "./ProfileViews.module.css";
import { cx } from "../lib/cx";

const JsonEditor = lazy(() =>
  import("../components/JsonEditor").then((module) => ({ default: module.JsonEditor })),
);

function ProfileTypeLabel(props: { type: DesktopProfileType }) {
  const { t } = useI18n();
  return <>{props.type === "remote" ? t("Remote") : t("Local")}</>;
}

function ProfileInfo(props: { profile: DesktopProfile }) {
  const { language } = useI18n();
  const now = useNow(60_000);
  const profile = props.profile;
  return (
    <span className={styles.profileInfo}>
      <span className={styles.profileInfoItem}>
        <Icon name={profile.type === "remote" ? "language" : "text_snippet"} size={12} />
        <ProfileTypeLabel type={profile.type} />
      </span>
      {profile.type === "remote" && profile.lastUpdated !== undefined && (
        <span className={styles.profileInfoItem}>
          <Icon name="schedule" size={12} />
          {formatRelativeTime(profile.lastUpdated, now, language)}
        </span>
      )}
    </span>
  );
}

function ShareMenuItems(props: {
  host: DesktopHost;
  profile: DesktopProfile;
  onShowQR: () => void;
  onShowQRS: () => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <MenuItem
        icon="save"
        onSelect={() => void props.host.profiles.exportData(props.profile.id).catch(showError)}
      >
        {t("Save File")}
      </MenuItem>
      <MenuItem
        icon="save"
        onSelect={() => void props.host.profiles.exportFile(props.profile.id).catch(showError)}
      >
        {t("Save Content JSON")}
      </MenuItem>
      {props.profile.type === "remote" && (
        <MenuItem icon="qr_code" onSelect={props.onShowQR}>
          {t("Share URL as QR Code")}
        </MenuItem>
      )}
      <MenuItem icon="qr_code" onSelect={props.onShowQRS}>
        {t("Share as QRS Code")}
      </MenuItem>
    </>
  );
}

export function ProfileCard(props: { host: DesktopHost }) {
  const host = props.host;
  const { t } = useI18n();
  const { selectedId, profiles } = useDesktopProfiles(host);
  const [picking, setPicking] = useState(false);
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState(false);
  const [showingQR, setShowingQR] = useState(false);
  const [showingQRS, setShowingQRS] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [importRequest, setImportRequest] = useState<{
    fileName: string;
    data: Uint8Array;
  } | null>(null);

  const selected = profiles.find((profile) => profile.id === selectedId) ?? null;

  const importFromFile = () => {
    host.profiles
      .pickImportFile()
      .then((request) => {
        if (request !== null) {
          setImportRequest(() => request);
        }
      })
      .catch(showError);
  };

  const updateRemote = () => {
    if (selected === null) {
      return;
    }
    setUpdating(true);
    host.profiles
      .updateRemote(selected.id)
      .catch(showError)
      .finally(() => setUpdating(false));
  };

  return (
    <Card
      icon="folder"
      title={t("Profile")}
      wide
      actions={
        <OthersMenu icon="add">
          <MenuItem icon="download" onSelect={importFromFile}>
            {t("Import from File")}
          </MenuItem>
          <MenuItem icon="edit" onSelect={() => setCreating(true)}>
            {t("Create Manually")}
          </MenuItem>
        </OthersMenu>
      }
    >
      {profiles.length === 0 ? (
        <p className={styles.profileEmpty}>{t("Empty profiles")}</p>
      ) : (
        <>
          <button type="button" className={styles.profileSelector} onClick={() => setPicking(true)}>
            <span className={styles.profileSelectorName}>
              {selected?.name ?? t("Select Profile")}
            </span>
            <Icon name="unfold_more" size={16} />
          </button>
          {selected !== null && (
            <>
              <div className={styles.profileMeta}>
                <ProfileInfo profile={selected} />
              </div>
              <div className={cx("row-actions", styles.profileActions)}>
                <IconButton title={t("Edit")} onClick={() => setEditing(true)}>
                  <Icon name="edit" size={16} />
                </IconButton>
                {selected.type === "remote" && (
                  <IconButton title={t("Update")} disabled={updating} onClick={updateRemote}>
                    <span className={updating ? styles.spin : undefined}>
                      <Icon name="sync" size={16} />
                    </span>
                  </IconButton>
                )}
                <OthersMenu icon="share">
                  <ShareMenuItems
                    host={host}
                    profile={selected}
                    onShowQR={() => setShowingQR(true)}
                    onShowQRS={() => setShowingQRS(true)}
                  />
                </OthersMenu>
              </div>
            </>
          )}
        </>
      )}
      {picking && <ProfilePickerDialog host={host} onClose={() => setPicking(false)} />}
      {creating && <CreateProfileDialog host={host} onClose={() => setCreating(false)} />}
      {importRequest !== null && (
        <ProfileImportDialog
          host={host}
          request={importRequest}
          onClose={() => setImportRequest(null)}
        />
      )}
      {editing && selected !== null && (
        <EditProfileDialog host={host} profile={selected} onClose={() => setEditing(false)} />
      )}
      {showingQR && selected !== null && (
        <ProfileQRDialog profile={selected} onClose={() => setShowingQR(false)} />
      )}
      {showingQRS && selected !== null && (
        <ProfileQRSDialog host={host} profile={selected} onClose={() => setShowingQRS(false)} />
      )}
    </Card>
  );
}

function ProfilePickerDialog(props: { host: DesktopHost; onClose: () => void }) {
  const host = props.host;
  const { t } = useI18n();
  const { selectedId, profiles } = useDesktopProfiles(host);
  const [editing, setEditing] = useState(false);
  const [editingProfile, setEditingProfile] = useState<DesktopProfile | null>(null);
  const [qrProfile, setQRProfile] = useState<DesktopProfile | null>(null);
  const [qrsProfile, setQRSProfile] = useState<DesktopProfile | null>(null);
  const [updatingId, setUpdatingId] = useState<string | null>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const select = (id: string) => {
    host.profiles
      .select(id)
      .then(props.onClose)
      .catch(showError);
  };

  const updateRemote = (profile: DesktopProfile) => {
    setUpdatingId(profile.id);
    host.profiles
      .updateRemote(profile.id)
      .catch(showError)
      .finally(() => setUpdatingId(null));
  };

  const moveDrag = (event: React.PointerEvent<HTMLElement>) => {
    if (dragIndex === null || !listRef.current) {
      return;
    }
    const rows = Array.from(listRef.current.children) as HTMLElement[];
    const target = rows.findIndex((row) => {
      const rect = row.getBoundingClientRect();
      return event.clientY >= rect.top && event.clientY < rect.bottom;
    });
    if (target >= 0 && target !== dragIndex) {
      const ids = profiles.map((profile) => profile.id);
      const [moved] = ids.splice(dragIndex, 1);
      ids.splice(target, 0, moved);
      host.profiles.reorder(ids).catch(showError);
      setDragIndex(target);
    }
  };

  if (editingProfile !== null) {
    return (
      <EditProfileDialog host={host} profile={editingProfile} onClose={() => setEditingProfile(null)} />
    );
  }
  if (qrProfile !== null) {
    return <ProfileQRDialog profile={qrProfile} onClose={() => setQRProfile(null)} />;
  }
  if (qrsProfile !== null) {
    return <ProfileQRSDialog host={host} profile={qrsProfile} onClose={() => setQRSProfile(null)} />;
  }

  return (
    <Dialog onClose={props.onClose} className={styles.profilePickerDialog}>
      <h3 className={styles.profilePickerHeader}>
        {t("Profiles")}
        {profiles.length > 0 && (
          <IconButton
            title={editing ? t("Done") : t("Edit")}
            active={editing}
            onClick={() => setEditing(!editing)}
          >
            <Icon name={editing ? "check" : "edit"} size={16} />
          </IconButton>
        )}
      </h3>
      <div className={styles.profileList} ref={listRef}>
        {profiles.map((profile, index) => (
          <div
            className={cx(styles.profileRow, dragIndex === index && styles.dragging)}
            key={profile.id}
          >
            {editing ? (
              <span
                className={styles.dragHandle}
                onPointerDown={(event) => {
                  event.preventDefault();
                  event.currentTarget.setPointerCapture(event.pointerId);
                  setDragIndex(index);
                }}
                onPointerMove={moveDrag}
                onPointerUp={() => setDragIndex(null)}
                onPointerCancel={() => setDragIndex(null)}
              >
                <Icon name="drag_handle" size={14} />
              </span>
            ) : (
              <span className={styles.profileCheck}>
                {profile.id === selectedId && <Icon name="check" size={16} />}
              </span>
            )}
            <button
              type="button"
              className={styles.profileRowMain}
              disabled={editing}
              onClick={() => select(profile.id)}
            >
              <span className={styles.profileRowName}>{profile.name}</span>
              <ProfileInfo profile={profile} />
            </button>
            <div className={styles.profileRowActions}>
              {editing ? (
                <IconButton
                  danger
                  title={t("Delete")}
                  onClick={() => void host.profiles.remove(profile.id).catch(showError)}
                >
                  <Icon name="delete" size={16} />
                </IconButton>
              ) : updatingId === profile.id ? (
                <span className={cx("icon-button", styles.spin)}>
                  <Icon name="sync" size={16} />
                </span>
              ) : (
                <OthersMenu icon="more_horiz">
                  <MenuItem icon="edit" onSelect={() => setEditingProfile(profile)}>
                    {t("Edit")}
                  </MenuItem>
                  {profile.type === "remote" && (
                    <MenuItem icon="sync" onSelect={() => updateRemote(profile)}>
                      {t("Update")}
                    </MenuItem>
                  )}
                  <SubMenu label={t("Share")} icon="share">
                    <ShareMenuItems
                      host={host}
                      profile={profile}
                      onShowQR={() => setQRProfile(profile)}
                      onShowQRS={() => setQRSProfile(profile)}
                    />
                  </SubMenu>
                </OthersMenu>
              )}
            </div>
          </div>
        ))}
      </div>
      <div className="row-actions dialog-actions">
        <Button variant="primary" onClick={props.onClose}>
          {t("Done")}
        </Button>
      </div>
    </Dialog>
  );
}

function CreateProfileDialog(props: {
  host: DesktopHost;
  title?: string;
  initial?: { name?: string; content?: string; type?: DesktopProfileType; remoteUrl?: string };
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [name, setName] = useState(props.initial?.name ?? "");
  const [type, setType] = useState<DesktopProfileType>(props.initial?.type ?? "local");
  const [remoteUrl, setRemoteUrl] = useState(props.initial?.remoteUrl ?? "");
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [interval, setIntervalValue] = useState("60");
  const [busy, setBusy] = useState(false);

  const valid = name.trim() !== "" && (type === "local" || remoteUrl.trim() !== "");

  const create = () => {
    setBusy(true);
    props.host.profiles
      .create({
        name: name.trim(),
        type,
        content: type === "local" ? props.initial?.content : undefined,
        remoteUrl: type === "remote" ? remoteUrl.trim() : undefined,
        autoUpdate: type === "remote" ? autoUpdate : false,
        autoUpdateIntervalMinutes: type === "remote" ? Number(interval) || 60 : undefined,
      })
      .then(props.onClose)
      .catch(showError)
      .finally(() => setBusy(false));
  };

  return (
    <Dialog onClose={props.onClose}>
      <h3>{props.title ?? t("New Profile")}</h3>
      <Field label={t("Name")}>
        <input className="input" value={name} onChange={(event) => setName(event.target.value)} />
      </Field>
      <Field label={t("Type")}>
        <Select<DesktopProfileType>
          options={[
            { value: "local", label: t("Local") },
            { value: "remote", label: t("Remote") },
          ]}
          value={type}
          onChange={setType}
        />
      </Field>
      {type === "remote" && (
        <>
          <Field label={t("URL")}>
            <input
              className="input"
              value={remoteUrl}
              onChange={(event) => setRemoteUrl(event.target.value)}
              placeholder="https://"
            />
          </Field>
          <Toggle label={t("Auto Update")} value={autoUpdate} onChange={setAutoUpdate} />
          <Field label={t("Auto Update Interval")}>
            <input
              className="input"
              type="number"
              min="15"
              placeholder={t("In Minutes")}
              value={interval}
              onChange={(event) => setIntervalValue(event.target.value)}
            />
          </Field>
        </>
      )}
      <div className="row-actions dialog-actions">
        <Button onClick={props.onClose}>
          {t("Cancel")}
        </Button>
        <Button variant="primary" disabled={!valid || busy} onClick={create}>
          {t("Create")}
        </Button>
      </div>
    </Dialog>
  );
}

function EditProfileDialog(props: {
  host: DesktopHost;
  profile: DesktopProfile;
  onClose: () => void;
}) {
  const { t, language } = useI18n();
  const now = useNow(60_000);
  const profile = props.profile;
  const [name, setName] = useState(profile.name);
  const [remoteUrl, setRemoteUrl] = useState(profile.remoteUrl ?? "");
  const [autoUpdate, setAutoUpdate] = useState(profile.autoUpdate);
  const [interval, setIntervalValue] = useState(String(profile.autoUpdateIntervalMinutes));
  const [editingContent, setEditingContent] = useState(false);
  const [confirmingClose, setConfirmingClose] = useState(false);
  const [busy, setBusy] = useState(false);

  const changed =
    name.trim() !== profile.name ||
    (profile.type === "remote" &&
      (remoteUrl.trim() !== (profile.remoteUrl ?? "") ||
        autoUpdate !== profile.autoUpdate ||
        (Number(interval) || profile.autoUpdateIntervalMinutes) !==
          profile.autoUpdateIntervalMinutes));

  const requestClose = () => {
    if (changed) {
      setConfirmingClose(true);
    } else {
      props.onClose();
    }
  };

  const save = () => {
    setBusy(true);
    props.host.profiles
      .updateMetadata(profile.id, {
        name: name.trim(),
        remoteUrl: profile.type === "remote" ? remoteUrl.trim() : undefined,
        autoUpdate,
        autoUpdateIntervalMinutes: Number(interval) || profile.autoUpdateIntervalMinutes,
      })
      .then(props.onClose)
      .catch(showError)
      .finally(() => setBusy(false));
  };

  if (editingContent) {
    return (
      <ProfileContentDialog
        host={props.host}
        profile={profile}
        readOnly={profile.type === "remote"}
        onClose={() => setEditingContent(false)}
      />
    );
  }

  return (
    <Dialog onClose={requestClose}>
      <h3>{t("Edit Profile")}</h3>
      <Field label={t("Name")}>
        <input className="input" value={name} onChange={(event) => setName(event.target.value)} />
      </Field>
      <Field label={t("Type")}>
        <input className="input" value={profile.type === "remote" ? t("Remote") : t("Local")} readOnly />
      </Field>
      {profile.type === "remote" && (
        <>
          <Field label={t("URL")}>
            <input
              className="input"
              value={remoteUrl}
              onChange={(event) => setRemoteUrl(event.target.value)}
            />
          </Field>
          <Toggle label={t("Auto Update")} value={autoUpdate} onChange={setAutoUpdate} />
          <Field label={t("Auto Update Interval")}>
            <input
              className="input"
              type="number"
              min="15"
              value={interval}
              onChange={(event) => setIntervalValue(event.target.value)}
            />
          </Field>
          {profile.lastUpdated !== undefined && (
            <div className={styles.profileMeta}>
              <span className={styles.profileInfoItem}>
                <Icon name="schedule" size={12} />
                {t("Last Updated")} · {formatRelativeTime(profile.lastUpdated, now, language)}
              </span>
            </div>
          )}
        </>
      )}
      <div className="row-actions dialog-actions">
        <Button
          style={{ marginInlineEnd: "auto" }}
          onClick={() => setEditingContent(true)}
        >
          {profile.type === "remote" ? t("View Content") : t("Edit Content")}
        </Button>
        <Button onClick={requestClose}>
          {t("Cancel")}
        </Button>
        <Button
          variant="primary"
          disabled={busy || !changed || name.trim() === ""}
          onClick={save}
        >
          {t("Save")}
        </Button>
      </div>
      {confirmingClose && (
        <Dialog onClose={() => setConfirmingClose(false)}>
          <h3>{t("Unsaved Changes")}</h3>
          <p className="dialog-message">
            {t("You have unsaved changes. Do you want to discard them?")}
          </p>
          <div className="row-actions dialog-actions">
            <Button onClick={() => setConfirmingClose(false)}>
              {t("Cancel")}
            </Button>
            <Button variant="danger" onClick={props.onClose}>
              {t("Discard")}
            </Button>
          </div>
        </Dialog>
      )}
    </Dialog>
  );
}

const EDITOR_SYMBOLS = ['"', ":", ",", "{", "}", "[", "]", "true", "false"];

function ProfileContentDialog(props: {
  host: DesktopHost;
  profile: DesktopProfile;
  readOnly: boolean;
  onClose: () => void;
}) {
  const host = props.host;
  const { t } = useI18n();
  const [content, setContent] = useState<string | null>(null);
  const [savedContent, setSavedContent] = useState<string | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [confirmingClose, setConfirmingClose] = useState(false);
  const [busy, setBusy] = useState(false);
  const checkTimer = useRef<number | null>(null);
  const editorRef = useRef<JsonEditorHandle>(null);

  useEffect(() => {
    host.profiles
      .readContent(props.profile.id)
      .then((value) => {
        setContent(() => value);
        setSavedContent(() => value);
      })
      .catch((error: unknown) => {
        showError(error);
        props.onClose();
      });
    return () => {
      if (checkTimer.current !== null) {
        window.clearTimeout(checkTimer.current);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.profile.id]);

  const edit = (value: string, undoAvailable: boolean, redoAvailable: boolean) => {
    setContent(() => value);
    setCanUndo(() => undoAvailable);
    setCanRedo(() => redoAvailable);
    setCheckError(null);
    if (checkTimer.current !== null) {
      window.clearTimeout(checkTimer.current);
    }
    if (value.trim() === "") {
      return;
    }
    checkTimer.current = window.setTimeout(() => {
      host.configuration.check(value).then(
        () => setCheckError(null),
        (error: unknown) => setCheckError(describeError(error).message),
      );
    }, 2000);
  };

  const format = () => {
    if (content === null) {
      return;
    }
    setBusy(true);
    host.configuration
      .format(content)
      .then((formatted) => {
        if (formatted !== content) {
          editorRef.current?.replaceAll(formatted);
        }
        setCheckError(null);
      })
      .catch((error: unknown) => setCheckError(describeError(error).message))
      .finally(() => setBusy(false));
  };

  const save = () => {
    if (content === null) {
      return;
    }
    setBusy(true);
    host.profiles
      .writeContent(props.profile.id, content)
      .then(props.onClose)
      .catch(showError)
      .finally(() => setBusy(false));
  };

  const changed = !props.readOnly && content !== null && content !== savedContent;

  const requestClose = () => {
    if (changed) {
      setConfirmingClose(true);
    } else {
      props.onClose();
    }
  };

  return (
    <Dialog onClose={requestClose} className={styles.profileEditorDialog}>
      <h3>{props.readOnly ? t("View Content") : t("Edit Content")}</h3>
      {savedContent === null ? (
        <div className={styles.profileEditor} />
      ) : (
        <Suspense fallback={<div className={styles.profileEditor} />}>
          <JsonEditor
            ref={editorRef}
            className={styles.profileEditor}
            initialValue={savedContent}
            readOnly={props.readOnly}
            onChange={edit}
            onSave={props.readOnly ? undefined : save}
          />
        </Suspense>
      )}
      {checkError !== null && (
        <div className={cx("banner error", styles.editorBanner)}>
          <span className={styles.editorBannerMessage}>{checkError}</span>
          <IconButton title={t("Close")} onClick={() => setCheckError(null)}>
            <Icon name="close" size={14} />
          </IconButton>
        </div>
      )}
      {!props.readOnly && savedContent !== null && (
        <div className={styles.editorToolbar} role="toolbar" aria-label={t("Edit Content")}>
          <button
            type="button"
            className={styles.editorKey}
            title={t("Undo")}
            aria-label={t("Undo")}
            disabled={!canUndo}
            onPointerDown={(event) => event.preventDefault()}
            onClick={() => editorRef.current?.undo()}
          >
            <Icon name="undo" size={16} />
          </button>
          <button
            type="button"
            className={styles.editorKey}
            title={t("Redo")}
            aria-label={t("Redo")}
            disabled={!canRedo}
            onPointerDown={(event) => event.preventDefault()}
            onClick={() => editorRef.current?.redo()}
          >
            <Icon name="redo" size={16} />
          </button>
          <button
            type="button"
            className={styles.editorKey}
            disabled={busy}
            onPointerDown={(event) => event.preventDefault()}
            onClick={format}
          >
            {t("Format")}
          </button>
          <span className={styles.editorToolbarDivider} aria-hidden="true" />
          {EDITOR_SYMBOLS.map((symbol) => (
            <button
              key={symbol}
              type="button"
              className={cx(styles.editorKey, styles.editorSymbol)}
              onPointerDown={(event) => event.preventDefault()}
              onClick={() => editorRef.current?.insertSymbol(symbol)}
            >
              {symbol}
            </button>
          ))}
        </div>
      )}
      <div className="row-actions dialog-actions">
        {props.readOnly ? (
          <>
            <Button
              disabled={content === null}
              onClick={() => {
                void navigator.clipboard.writeText(content ?? "").catch(() => {});
              }}
            >
              {t("Copy")}
            </Button>
            <Button variant="primary" onClick={props.onClose}>
              {t("Close")}
            </Button>
          </>
        ) : (
          <>
            <Button onClick={requestClose}>
              {t("Cancel")}
            </Button>
            <Button variant="primary" disabled={busy || content === null} onClick={save}>
              {t("Save")}
            </Button>
          </>
        )}
      </div>
      {confirmingClose && (
        <Dialog onClose={() => setConfirmingClose(false)}>
          <h3>{t("Unsaved Changes")}</h3>
          <p className="dialog-message">{t("Do you want to save the changes you made?")}</p>
          <div className="row-actions dialog-actions">
            <Button variant="danger" onClick={props.onClose}>
              {t("Don't Save")}
            </Button>
            <Button onClick={() => setConfirmingClose(false)}>
              {t("Cancel")}
            </Button>
            <Button variant="primary" disabled={busy} onClick={save}>
              {t("Save")}
            </Button>
          </div>
        </Dialog>
      )}
    </Dialog>
  );
}

function ProfileQRDialog(props: { profile: DesktopProfile; onClose: () => void }) {
  const { t } = useI18n();
  const profile = props.profile;
  const importLink =
    profile.remoteUrl !== undefined
      ? `sing-box://import-remote-profile?url=${encodeURIComponent(profile.remoteUrl)}#${encodeURIComponent(profile.name)}`
      : null;

  return (
    <Dialog onClose={props.onClose}>
      <h3>{t("Share URL as QR Code")}</h3>
      {importLink !== null && <QRCode value={importLink} />}
      <div className="row-actions dialog-actions">
        <Button variant="primary" onClick={props.onClose}>
          {t("Close")}
        </Button>
      </div>
    </Dialog>
  );
}

export function SystemProxyCard(props: { host: DesktopHost }) {
  const host = props.host;
  const { t } = useI18n();
  const [status, setStatus] = useState<{ available: boolean; enabled: boolean } | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let stale = false;
    host.systemProxy
      .status()
      .then((value) => {
        if (!stale) {
          setStatus(() => value);
        }
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [host]);

  if (status === null || !status.available) {
    return null;
  }

  return (
    <Card
      icon="router"
      title={t("System HTTP Proxy")}
      wide
      actions={
        <Switch
          label={t("System HTTP Proxy")}
          value={status.enabled}
          disabled={busy}
          onChange={(value) => {
            setBusy(true);
            host.systemProxy
              .setEnabled(value)
              .then(() => host.systemProxy.status())
              .then(setStatus)
              .catch(showError)
              .finally(() => setBusy(false));
          }}
        />
      }
    />
  );
}

export function ImportRemoteProfileDialog(props: { host: DesktopHost }) {
  const host = props.host;
  const { t } = useI18n();
  const [request, setRequest] = useState<{ name: string; url: string } | null>(null);
  const [confirmed, setConfirmed] = useState(false);

  useEffect(
    () =>
      host.onImportRemoteProfile((value) => {
        setRequest(() => value);
        setConfirmed(false);
      }),
    [host],
  );

  if (request === null) {
    return null;
  }

  if (confirmed) {
    return (
      <CreateProfileDialog
        host={host}
        title={t("Import Profile")}
        initial={{ name: request.name, type: "remote", remoteUrl: request.url }}
        onClose={() => setRequest(null)}
      />
    );
  }

  let downloadHost: string;
  try {
    downloadHost = new URL(request.url).host;
  } catch {
    downloadHost = request.url;
  }

  return (
    <Dialog onClose={() => setRequest(null)}>
      <h3>{t("Import Remote Profile")}</h3>
      <p className="dialog-message">
        {t(
          "Are you sure to import remote profile {name}? You will connect to {host} to download the configuration.",
          { name: request.name, host: downloadHost },
        )}
      </p>
      <div className="row-actions dialog-actions">
        <Button onClick={() => setRequest(null)}>
          {t("Cancel")}
        </Button>
        <Button
          variant="primary"
          onClick={() => {
            navigate("overview");
            setConfirmed(true);
          }}
        >
          {t("Import")}
        </Button>
      </div>
    </Dialog>
  );
}

function ProfileImportDialog(props: {
  host: DesktopHost;
  request: { fileName: string; data: Uint8Array };
  onClose: () => void;
}) {
  const host = props.host;
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState<string | null>(null);
  const fileName = props.request.fileName;
  const isConfiguration = fileName.toLowerCase().endsWith(".json");

  useEffect(() => {
    if (isConfiguration) {
      return;
    }
    host.profiles
      .decodeData(props.request.data)
      .then((decoded) => setName(decoded.name || fileName.replace(/\.bpf$/i, "")))
      .catch((error: unknown) => {
        showError(error);
        props.onClose();
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.request]);

  if (isConfiguration) {
    return (
      <CreateProfileDialog
        host={host}
        initial={{
          name: fileName.slice(0, -".json".length),
          content: new TextDecoder().decode(props.request.data),
        }}
        onClose={props.onClose}
      />
    );
  }

  if (name === null) {
    return null;
  }

  const importProfile = () => {
    setBusy(true);
    host.profiles
      .importData(fileName, props.request.data)
      .then(() => {
        props.onClose();
        navigate("overview");
      })
      .catch(showError)
      .finally(() => setBusy(false));
  };

  return (
    <Dialog onClose={props.onClose}>
      <h3>{t("Import Profile")}</h3>
      <p className="dialog-message">{t("Are you sure to import profile {name}?", { name })}</p>
      <div className="row-actions dialog-actions">
        <Button disabled={busy} onClick={props.onClose}>
          {t("Cancel")}
        </Button>
        <Button variant="primary" disabled={busy} onClick={importProfile}>
          {t("Import")}
        </Button>
      </div>
    </Dialog>
  );
}

export function ImportProfileFileDialog(props: { host: DesktopHost }) {
  const host = props.host;
  const [request, setRequest] = useState<{ fileName: string; data: Uint8Array } | null>(null);

  useEffect(() => host.onImportProfileFile(setRequest), [host]);

  if (request === null) {
    return null;
  }

  return <ProfileImportDialog host={host} request={request} onClose={() => setRequest(null)} />;
}
