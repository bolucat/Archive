import type { MessageInitShape } from "@bufbuild/protobuf";
import { ConnectError } from "@connectrpc/connect";
import type { Client } from "@connectrpc/connect";
import { createWritableIterable } from "@connectrpc/connect/protocol";
import { BrowserWindow, dialog, ipcMain, shell } from "electron";
import { createReadStream } from "node:fs";
import { mkdtemp, open, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

import { TAILDROP_CHUNK_SIZE } from "@dashboard/lib/taildrop";

import {
  TAILDROP_DOWNLOAD,
  TAILDROP_DOWNLOAD_PROGRESS,
  TAILDROP_SEND_CANCEL,
  TAILDROP_SEND_EVENT,
  TAILDROP_SEND_START,
} from "../shared/ipc";
import type {
  ProfilesResult,
  TaildropDownloadRequest,
  TaildropSendEvent,
  TaildropSendFile,
  TaildropSendRequest,
} from "../shared/ipc";
import {
  StartedService,
  type TaildropSendClientMessageSchema,
} from "../shared/gen/daemon/started_service_pb";
import { startedService } from "./daemon";
import { safeFileName, scheduleTemporaryRemoval, shareFilePath } from "./sharing";

// Electron rebuilds the argv of a second instance from the Chromium command line,
// which moves every switch ahead of the positional arguments.
const SEND_ARGUMENT = "--taildrop-send=";

// Explorer splits a large selection into several invocations of the verb despite
// MultiSelectModel.
const BATCH_WINDOW_MILLISECONDS = 250;

export function taildropSendPaths(argv: string[], workingDirectory: string): string[] {
  return argv
    .filter((argument) => argument.startsWith(SEND_ARGUMENT))
    .map((argument) => argument.slice(SEND_ARGUMENT.length))
    .filter((path) => path !== "")
    .map((path) => resolve(workingDirectory, path));
}

async function describeFile(path: string): Promise<TaildropSendFile | null> {
  try {
    const information = await stat(path);
    if (!information.isFile()) {
      return null;
    }
    return { name: basename(path), size: information.size, path };
  } catch {
    return null;
  }
}

export function createTaildropSendBatcher(
  deliver: (files: TaildropSendFile[]) => void,
): (paths: string[]) => void {
  const pending = new Set<string>();
  let timer: NodeJS.Timeout | null = null;

  const flush = () => {
    timer = null;
    const paths = [...pending];
    pending.clear();
    void Promise.all(paths.map(describeFile)).then((files) => {
      const present = files.filter((file) => file !== null);
      if (present.length > 0) {
        deliver(present);
      }
    });
  };

  return (paths) => {
    if (paths.length === 0) {
      return;
    }
    for (const path of paths) {
      pending.add(path);
    }
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(flush, BATCH_WINDOW_MILLISECONDS);
  };
}

function requireStartedService(): Client<typeof StartedService> {
  if (startedService === null) {
    throw new Error("daemon socket is not configured");
  }
  return startedService;
}

async function pushFiles(
  client: Client<typeof StartedService>,
  request: TaildropSendRequest,
  signal: AbortSignal,
  onProgress: (fileIndex: number, sentBytes: number, fileCompleted: boolean) => void,
): Promise<void> {
  const sizes: number[] = [];
  for (const file of request.files) {
    const information = await stat(file.path);
    if (!information.isFile()) {
      throw new Error(`not a regular file: ${file.path}`);
    }
    sizes.push(information.size);
  }
  const input = createWritableIterable<MessageInitShape<typeof TaildropSendClientMessageSchema>>();
  const responses = client.sendTaildropFiles(input, { signal });
  let pushError: unknown;
  const pushing = (async () => {
    try {
      await input.write({
        message: {
          case: "start",
          value: {
            endpointTag: request.endpointTag,
            peerStableID: request.peerStableID,
            files: request.files.map((file, index) => ({
              name: file.name,
              size: BigInt(sizes[index]),
            })),
          },
        },
      });
      for (const file of request.files) {
        const content = createReadStream(file.path, {
          highWaterMark: TAILDROP_CHUNK_SIZE,
          signal,
        });
        for await (const chunk of content) {
          await input.write({ message: { case: "chunk", value: { data: chunk as Uint8Array } } });
        }
        await input.write({ message: { case: "fileDone", value: {} } });
      }
    } finally {
      input.close();
    }
  })().catch((error: unknown) => {
    pushError = error;
  });
  let failure: unknown;
  try {
    for await (const message of responses) {
      if (message.message.case !== "progress") {
        continue;
      }
      const progress = message.message.value;
      onProgress(progress.fileIndex, Number(progress.sentBytes), progress.fileCompleted);
    }
  } catch (error) {
    failure = error;
  }
  input.close();
  await pushing;
  if (failure !== undefined) {
    // When the daemon ends the call with a status, pending writes reject with
    // a stream error that hides it; only a local error outranks the status.
    if (pushError !== undefined && !(pushError instanceof ConnectError)) {
      throw pushError;
    }
    throw failure;
  }
}

async function receiveFile(
  client: Client<typeof StartedService>,
  endpointTag: string,
  name: string,
  destination: string,
  onProgress: (transferred: number, size: number) => void,
): Promise<void> {
  const handle = await open(destination, "w");
  let size = -1;
  let transferred = 0;
  try {
    for await (const chunk of client.downloadTaildropFile({ endpointTag, name })) {
      if (size < 0) {
        size = Number(chunk.size);
        onProgress(0, size);
      }
      if (chunk.data.length === 0) {
        continue;
      }
      await handle.write(chunk.data);
      transferred += chunk.data.length;
      onProgress(transferred, size);
    }
  } finally {
    await handle.close();
  }
  if (transferred !== size) {
    throw new Error(`incomplete download: ${transferred} of ${size} bytes`);
  }
}

async function saveDownload(
  client: Client<typeof StartedService>,
  window: BrowserWindow,
  request: TaildropDownloadRequest,
  onProgress: (transferred: number, size: number) => void,
): Promise<boolean> {
  const result = await dialog.showSaveDialog(window, {
    defaultPath: safeFileName(request.name),
  });
  if (result.canceled || !result.filePath) {
    return false;
  }
  const destination = result.filePath;
  try {
    await receiveFile(client, request.endpointTag, request.name, destination, onProgress);
  } catch (error) {
    await rm(destination, { force: true }).catch(() => undefined);
    throw error;
  }
  return true;
}

async function openOrShareDownload(
  client: Client<typeof StartedService>,
  window: BrowserWindow,
  request: TaildropDownloadRequest,
  onProgress: (transferred: number, size: number) => void,
): Promise<boolean> {
  const name = safeFileName(request.name);
  const directory = await mkdtemp(join(tmpdir(), "sing-box-taildrop-"));
  const path = join(directory, name);
  try {
    await receiveFile(client, request.endpointTag, request.name, path, onProgress);
    if (request.action === "open") {
      const failure = await shell.openPath(path);
      if (failure !== "") {
        throw new Error(failure);
      }
    } else {
      await shareFilePath(window, path, name);
    }
  } catch (error) {
    await rm(directory, { recursive: true, force: true }).catch(() => undefined);
    throw error;
  }
  scheduleTemporaryRemoval(directory);
  return true;
}

export function registerTaildrop() {
  const sendSessions = new Map<number, { controller: AbortController; senderID: number }>();
  const watchedSenders = new Set<number>();

  ipcMain.on(TAILDROP_SEND_START, (event, sessionID: number, request: TaildropSendRequest) => {
    const sender = event.sender;
    const controller = new AbortController();
    sendSessions.set(sessionID, { controller, senderID: sender.id });
    if (!watchedSenders.has(sender.id)) {
      watchedSenders.add(sender.id);
      sender.once("destroyed", () => {
        watchedSenders.delete(sender.id);
        for (const [id, session] of sendSessions) {
          if (session.senderID === sender.id) {
            session.controller.abort();
            sendSessions.delete(id);
          }
        }
      });
    }
    const emit = (payload: TaildropSendEvent) => {
      if (!sender.isDestroyed()) {
        sender.send(TAILDROP_SEND_EVENT, payload);
      }
    };
    void (async () => {
      try {
        await pushFiles(
          requireStartedService(),
          request,
          controller.signal,
          (fileIndex, sentBytes, fileCompleted) =>
            emit({ sessionID, type: "progress", fileIndex, sentBytes, fileCompleted }),
        );
        emit({ sessionID, type: "finished", code: 0, error: "" });
      } catch (error) {
        if (!controller.signal.aborted) {
          const failure = ConnectError.from(error);
          emit({ sessionID, type: "finished", code: failure.code, error: failure.rawMessage });
        }
      } finally {
        sendSessions.delete(sessionID);
      }
    })();
  });

  ipcMain.on(TAILDROP_SEND_CANCEL, (_event, sessionID: number) => {
    sendSessions.get(sessionID)?.controller.abort();
    sendSessions.delete(sessionID);
  });

  ipcMain.handle(
    TAILDROP_DOWNLOAD,
    async (event, request: TaildropDownloadRequest): Promise<ProfilesResult> => {
      try {
        const window = BrowserWindow.fromWebContents(event.sender);
        if (window === null) {
          throw new Error("download window is unavailable");
        }
        const client = requireStartedService();
        const onProgress = (transferred: number, size: number) => {
          if (!event.sender.isDestroyed()) {
            event.sender.send(TAILDROP_DOWNLOAD_PROGRESS, {
              downloadID: request.downloadID,
              transferred,
              size,
            });
          }
        };
        const saved =
          request.action === "save"
            ? await saveDownload(client, window, request, onProgress)
            : await openOrShareDownload(client, window, request, onProgress);
        return { ok: true, value: saved };
      } catch (error) {
        return { ok: false, error: error instanceof Error ? error.message : String(error) };
      }
    },
  );
}
