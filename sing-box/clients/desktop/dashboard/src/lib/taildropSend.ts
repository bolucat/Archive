import { useSyncExternalStore } from "react";

import type { DaemonApi } from "../api/daemon";
import type { DesktopHost, DesktopTaildropFile } from "../app/desktop";
import { StartedService } from "../gen/daemon/started_service_pb";
import { TAILDROP_CHUNK_SIZE } from "./taildrop";

const SEND_WINDOW = 4 * 1024 * 1024;
const SEND_WAIT_TIMEOUT = 1000;

export interface TaildropSendFileState {
  name: string;
  size: number;
  sentBytes: number;
  completed: boolean;
}

export interface TaildropSendSessionState {
  id: number;
  api: DaemonApi;
  endpointTag: string;
  peerName: string;
  files: TaildropSendFileState[];
  finished: boolean;
  error: string;
}

interface Session {
  state: TaildropSendSessionState;
  cancel: () => void;
}

const sessions: Session[] = [];
const listeners = new Set<() => void>();
let snapshot: TaildropSendSessionState[] = [];
let sequence = 0;
let subscribedHost: DesktopHost | null = null;

function publish() {
  snapshot = sessions.map((session) => session.state);
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): TaildropSendSessionState[] {
  return snapshot;
}

function updateSession(
  id: number,
  mutate: (state: TaildropSendSessionState) => TaildropSendSessionState,
) {
  const index = sessions.findIndex((session) => session.state.id === id);
  if (index < 0) {
    return;
  }
  sessions[index] = { ...sessions[index], state: mutate(sessions[index].state) };
  publish();
}

function applyProgress(id: number, fileIndex: number, sentBytes: number, fileCompleted: boolean) {
  updateSession(id, (state) => {
    if (fileIndex < 0 || fileIndex >= state.files.length) {
      return state;
    }
    return {
      ...state,
      files: state.files.map((file, at) =>
        at === fileIndex
          ? {
              ...file,
              sentBytes: fileCompleted ? file.size : sentBytes,
              completed: file.completed || fileCompleted,
            }
          : file,
      ),
    };
  });
}

function failureMessage(code: number, message: string): string {
  if (code === 0) {
    return "";
  }
  return message || `stream closed with status ${code}`;
}

function finishSession(id: number, error: string) {
  updateSession(id, (state) => ({
    ...state,
    finished: true,
    error: error !== "" ? error : state.error,
  }));
}

function createSession(
  api: DaemonApi,
  endpointTag: string,
  peerName: string,
  files: { name: string; size: number }[],
): Session {
  sequence += 1;
  const session: Session = {
    state: {
      id: sequence,
      api,
      endpointTag,
      peerName,
      files: files.map((file) => ({
        name: file.name,
        size: file.size,
        sentBytes: 0,
        completed: false,
      })),
      finished: false,
      error: "",
    },
    cancel: () => {},
  };
  sessions.push(session);
  publish();
  return session;
}

export function useTaildropSendSessions(
  api: DaemonApi,
  endpointTag: string,
): TaildropSendSessionState[] {
  const all = useSyncExternalStore(subscribe, getSnapshot);
  return all.filter((session) => session.api === api && session.endpointTag === endpointTag);
}

export function useTaildropSendFailures(api: DaemonApi): TaildropSendSessionState[] {
  const all = useSyncExternalStore(subscribe, getSnapshot);
  return all.filter((session) => session.api === api && session.error !== "");
}

export function dismissTaildropSend(id: number): void {
  const index = sessions.findIndex((session) => session.state.id === id);
  if (index < 0) {
    return;
  }
  const session = sessions[index];
  session.cancel();
  sessions.splice(index, 1);
  publish();
}

export function startTaildropSend(
  api: DaemonApi,
  endpointTag: string,
  peerStableID: string,
  peerName: string,
  files: File[],
): void {
  if (files.length === 0) {
    return;
  }
  const session = createSession(api, endpointTag, peerName, files);
  const id = session.state.id;

  let stopped = false;
  let received = 0;
  let pushed = 0;
  let resume: (() => void) | null = null;
  const wake = () => {
    const pending = resume;
    resume = null;
    pending?.();
  };
  const waitForWindow = () =>
    new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        resume = null;
        resolve();
      }, SEND_WAIT_TIMEOUT);
      resume = () => {
        clearTimeout(timer);
        resolve();
      };
    });

  const stream = api.openBidirectionalStream(StartedService.method.sendTaildropFiles, {
    onMessage: (message) => {
      if (stopped) {
        return;
      }
      if (message.message.case === "receivedBytes") {
        received = Number(message.message.value);
        wake();
        return;
      }
      if (message.message.case !== "progress") {
        return;
      }
      const progress = message.message.value;
      const index = progress.fileIndex;
      if (index >= 0 && index < files.length) {
        const fileSent = progress.fileCompleted ? files[index].size : Number(progress.sentBytes);
        applyProgress(id, index, fileSent, progress.fileCompleted);
      }
    },
    onEnd: (status, endError) => {
      if (stopped) {
        return;
      }
      stopped = true;
      wake();
      let message = "";
      if (status) {
        message = failureMessage(status.code, status.message);
      } else if (endError) {
        message = endError;
      }
      finishSession(id, message);
    },
  });

  session.cancel = () => {
    if (stopped) {
      return;
    }
    stopped = true;
    wake();
    stream.close();
  };

  stream.send({
    message: {
      case: "start",
      value: {
        endpointTag,
        peerStableID,
        files: files.map((file) => ({ name: file.name, size: BigInt(file.size) })),
      },
    },
  });

  void (async () => {
    try {
      for (const file of files) {
        let offset = 0;
        while (offset < file.size) {
          while (!stopped && pushed - received >= SEND_WINDOW) {
            await waitForWindow();
          }
          if (stopped) {
            return;
          }
          const end = Math.min(offset + TAILDROP_CHUNK_SIZE, file.size);
          const data = new Uint8Array(await file.slice(offset, end).arrayBuffer());
          if (stopped) {
            return;
          }
          stream.send({ message: { case: "chunk", value: { data } } });
          offset = end;
          pushed += data.length;
        }
        if (stopped) {
          return;
        }
        stream.send({ message: { case: "fileDone", value: {} } });
      }
    } catch (readError) {
      if (!stopped) {
        stopped = true;
        wake();
        stream.close();
        finishSession(id, String(readError));
      }
    }
  })();
}

export function startDesktopTaildropSend(
  api: DaemonApi,
  host: DesktopHost,
  endpointTag: string,
  peerStableID: string,
  peerName: string,
  files: DesktopTaildropFile[],
): void {
  if (files.length === 0) {
    return;
  }
  if (subscribedHost !== host) {
    subscribedHost = host;
    host.taildrop.onSendEvent((event) => {
      if (event.type === "progress") {
        applyProgress(event.sessionID, event.fileIndex, event.sentBytes, event.fileCompleted);
      } else {
        finishSession(event.sessionID, failureMessage(event.code, event.error));
      }
    });
  }
  const session = createSession(api, endpointTag, peerName, files);
  const id = session.state.id;
  session.cancel = () => host.taildrop.cancelSend(id);
  host.taildrop.startSend(id, { endpointTag, peerStableID, files });
}
