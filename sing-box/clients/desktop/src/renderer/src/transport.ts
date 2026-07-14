import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import type { Transport } from "@connectrpc/connect";

import type { StreamEvent } from "@shared/ipc";

const streamHandlers = new Map<number, (event: StreamEvent) => void>();
let nextStreamId = Math.floor(Math.random() * 2 ** 30) * 1024;
let subscribed = false;

export function createIpcTransport(): Transport {
  if (!subscribed) {
    subscribed = true;
    window.desktop.daemon.onStreamEvent((event) => {
      streamHandlers.get(event.id)?.(event);
    });
  }
  return {
    async unary(method, signal, _timeoutMs, _header, input) {
      signal?.throwIfAborted();
      const result = await window.desktop.daemon.unary(
        method.parent.typeName,
        method.name,
        toBinary(method.input, create(method.input, input)),
      );
      if (!result.ok) {
        throw new ConnectError(result.error.message, result.error.code);
      }
      return {
        stream: false,
        service: method.parent,
        method,
        header: new Headers(),
        trailer: new Headers(),
        message: fromBinary(method.output, result.data),
      };
    },
    async stream(method, signal, _timeoutMs, _header, input) {
      if (method.methodKind !== "server_streaming") {
        throw new ConnectError("only server streaming is bridged", Code.Unimplemented);
      }
      signal?.throwIfAborted();
      let request: MessageInitShape<typeof method.input> | undefined;
      for await (const message of input) {
        request = message;
        break;
      }
      const id = nextStreamId++;
      const queue: StreamEvent[] = [];
      let notify: (() => void) | null = null;
      streamHandlers.set(id, (event) => {
        queue.push(event);
        notify?.();
        notify = null;
      });
      const abort = () => {
        window.desktop.daemon.streamCancel(id);
        streamHandlers.get(id)?.({
          id,
          type: "end",
          error: { code: Code.Canceled, message: "stream canceled" },
        });
      };
      signal?.addEventListener("abort", abort, { once: true });
      window.desktop.daemon.streamOpen(
        id,
        method.parent.typeName,
        method.name,
        toBinary(method.input, create(method.input, request)),
      );
      const output = method.output;
      async function* messages() {
        try {
          for (;;) {
            while (queue.length === 0) {
              await new Promise<void>((resolve) => {
                notify = resolve;
              });
            }
            const event = queue.shift()!;
            if (event.type === "message") {
              yield fromBinary(output, event.data);
              continue;
            }
            if (event.error) {
              throw new ConnectError(event.error.message, event.error.code);
            }
            return;
          }
        } finally {
          streamHandlers.delete(id);
          signal?.removeEventListener("abort", abort);
        }
      }
      return {
        stream: true,
        service: method.parent,
        method,
        header: new Headers(),
        trailer: new Headers(),
        message: messages(),
      };
    },
  };
}
