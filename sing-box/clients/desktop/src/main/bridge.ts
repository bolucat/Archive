import { fromBinary, toBinary } from "@bufbuild/protobuf";
import type { DescMethod, DescMethodStreaming, DescMethodUnary, DescService } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { createWritableIterable } from "@connectrpc/connect/protocol";
import { BrowserWindow, ipcMain } from "electron";
import type { WebContents } from "electron";

import {
  DAEMON_RETRY,
  DAEMON_STATE_CHANGED,
  DAEMON_STATE_GET,
  DAEMON_STREAM_CANCEL,
  DAEMON_STREAM_END,
  DAEMON_STREAM_EVENT,
  DAEMON_STREAM_OPEN,
  DAEMON_STREAM_SEND,
  DAEMON_UNARY,
} from "../shared/ipc";
import type { BridgeError, StreamEvent, UnaryResult } from "../shared/ipc";
import { ManagedService } from "../shared/gen/daemon/managed_service_pb";
import { StartedService } from "../shared/gen/daemon/started_service_pb";
import { ApplicationService, DesktopService } from "../shared/gen/experimental/boxdd/desktop_service_pb";
import { daemonTransport } from "./daemon";
import { daemonState } from "./state";
import { workerTransport } from "./worker";

const services: DescService[] = [StartedService, ManagedService, DesktopService, ApplicationService];

function serviceTransport(serviceName: string) {
  if (serviceName === ApplicationService.typeName) {
    return workerTransport;
  }
  return daemonTransport;
}

function findMethod(serviceName: string, methodName: string): DescMethod | undefined {
  const service = services.find((candidate) => candidate.typeName === serviceName);
  return service?.methods.find((candidate) => candidate.name === methodName);
}

function bridgeError(error: unknown): BridgeError {
  const connectError = ConnectError.from(error);
  return { code: connectError.code, message: connectError.rawMessage };
}

class StreamInput {
  private readonly input = createWritableIterable<Uint8Array>();
  private ended = false;

  send(message: Uint8Array) {
    if (this.ended) {
      return;
    }
    void this.input.write(message).catch(() => {});
  }

  end() {
    if (this.ended) {
      return;
    }
    this.ended = true;
    this.input.close();
  }

  async *messages(method: DescMethodStreaming) {
    for await (const message of this.input) {
      yield fromBinary(method.input, message);
    }
  }
}

interface ActiveStream {
  controller: AbortController;
  input: StreamInput;
}

export function registerDaemonBridge() {
  const streams = new Map<number, Map<number, ActiveStream>>();

  const senderStreams = (sender: WebContents): Map<number, ActiveStream> => {
    const existing = streams.get(sender.id);
    if (existing !== undefined) {
      return existing;
    }
    const created = new Map<number, ActiveStream>();
    streams.set(sender.id, created);
    sender.once("destroyed", () => {
      for (const stream of created.values()) {
        stream.input.end();
        stream.controller.abort();
      }
      if (streams.get(sender.id) === created) {
        streams.delete(sender.id);
      }
    });
    return created;
  };

  const unavailable: BridgeError = {
    code: Code.Unavailable,
    message: "daemon socket is not configured",
  };

  ipcMain.handle(
    DAEMON_UNARY,
    async (
      _event,
      serviceName: string,
      methodName: string,
      header: [string, string][],
      request: Uint8Array,
    ): Promise<UnaryResult> => {
      const method = findMethod(serviceName, methodName);
      if (method === undefined || method.methodKind !== "unary") {
        return { ok: false, error: { code: Code.Unimplemented, message: "unknown method" } };
      }
      const transport = serviceTransport(serviceName);
      if (transport === null) {
        return { ok: false, error: unavailable };
      }
      try {
        const unaryMethod = method as DescMethodUnary;
        const input = fromBinary(unaryMethod.input, request);
        const response = await transport.unary(
          unaryMethod,
          undefined,
          undefined,
          new Headers(header),
          input,
        );
        return { ok: true, data: toBinary(unaryMethod.output, response.message) };
      } catch (error) {
        return { ok: false, error: bridgeError(error) };
      }
    },
  );

  ipcMain.on(
    DAEMON_STREAM_OPEN,
    (
      event,
      id: number,
      serviceName: string,
      methodName: string,
      header: [string, string][],
    ) => {
      const sender = event.sender;
      const send = (payload: StreamEvent) => {
        if (!sender.isDestroyed()) {
          sender.send(DAEMON_STREAM_EVENT, payload);
        }
      };
      const method = findMethod(serviceName, methodName);
      if (method === undefined || method.methodKind === "unary") {
        send({ id, type: "end", error: { code: Code.Unimplemented, message: "unknown method" } });
        return;
      }
      const transport = serviceTransport(serviceName);
      if (transport === null) {
        send({ id, type: "end", error: unavailable });
        return;
      }
      const controller = new AbortController();
      const input = new StreamInput();
      const active: ActiveStream = { controller, input };
      const activeStreams = senderStreams(sender);
      const previous = activeStreams.get(id);
      if (previous !== undefined) {
        previous.input.end();
        previous.controller.abort();
      }
      activeStreams.set(id, active);
      void (async () => {
        try {
          const streamingMethod = method as DescMethodStreaming;
          const response = await transport.stream(
            streamingMethod,
            controller.signal,
            undefined,
            new Headers(header),
            input.messages(streamingMethod),
          );
          for await (const message of response.message) {
            send({ id, type: "message", data: toBinary(streamingMethod.output, message) });
          }
          send({ id, type: "end" });
        } catch (error) {
          if (!controller.signal.aborted) {
            send({ id, type: "end", error: bridgeError(error) });
          }
        } finally {
          input.end();
          if (activeStreams.get(id) === active) {
            activeStreams.delete(id);
          }
          if (activeStreams.size === 0 && streams.get(sender.id) === activeStreams) {
            streams.delete(sender.id);
          }
        }
      })();
    },
  );

  ipcMain.on(DAEMON_STREAM_SEND, (event, id: number, request: Uint8Array) => {
    streams.get(event.sender.id)?.get(id)?.input.send(request);
  });

  ipcMain.on(DAEMON_STREAM_END, (event, id: number) => {
    streams.get(event.sender.id)?.get(id)?.input.end();
  });

  ipcMain.on(DAEMON_STREAM_CANCEL, (event, id: number) => {
    const activeStreams = streams.get(event.sender.id);
    const active = activeStreams?.get(id);
    active?.input.end();
    active?.controller.abort();
    activeStreams?.delete(id);
    if (activeStreams?.size === 0) {
      streams.delete(event.sender.id);
    }
  });

  ipcMain.handle(DAEMON_STATE_GET, () => daemonState.connection);

  ipcMain.on(DAEMON_RETRY, () => daemonState.retryConnection());

  daemonState.on("connection", () => {
    for (const window of BrowserWindow.getAllWindows()) {
      if (!window.webContents.isDestroyed()) {
        window.webContents.send(DAEMON_STATE_CHANGED, daemonState.connection);
      }
    }
  });
}
