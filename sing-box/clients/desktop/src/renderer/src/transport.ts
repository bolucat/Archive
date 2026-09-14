import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import type { Transport } from "@connectrpc/connect";
import { createWritableIterable } from "@connectrpc/connect/protocol";

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
    async unary(method, signal, _timeoutMs, header, input) {
      signal?.throwIfAborted();
      const result = await window.desktop.daemon.unary(
        method.parent.typeName,
        method.name,
        Array.from(new Headers(header).entries()),
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
    async stream(method, signal, _timeoutMs, header, input) {
      signal?.throwIfAborted();
      const id = nextStreamId++;
      const events = createWritableIterable<StreamEvent>();
      let completed = false;
      let disposed = false;
      let inputError: unknown;
      const finish = (event: StreamEvent, error?: unknown) => {
        if (completed || disposed) {
          return;
        }
        completed = true;
        inputError = error;
        void events.write(event).catch(() => {});
        events.close();
      };
      streamHandlers.set(id, (event) => {
        if (completed || disposed) {
          return;
        }
        if (event.type === "end") {
          finish(event);
          return;
        }
        void events.write(event).catch(() => {});
      });
      const abort = () => {
        window.desktop.daemon.streamCancel(id);
        finish(
          { id, type: "end" },
          new ConnectError("stream canceled", Code.Canceled),
        );
      };
      signal?.addEventListener("abort", abort, { once: true });
      window.desktop.daemon.streamOpen(
        id,
        method.parent.typeName,
        method.name,
        Array.from(new Headers(header).entries()),
      );
      const inputIterator = input[Symbol.asyncIterator]();
      void (async () => {
        try {
          for (;;) {
            const next = await inputIterator.next();
            if (next.done) {
              break;
            }
            if (completed || disposed) {
              return;
            }
            window.desktop.daemon.streamSend(
              id,
              toBinary(method.input, create(method.input, next.value)),
            );
          }
          if (!completed && !disposed) {
            window.desktop.daemon.streamEnd(id);
          }
        } catch (error) {
          if (!completed && !disposed) {
            window.desktop.daemon.streamCancel(id);
            finish({ id, type: "end" }, error);
          }
        }
      })();
      const output = method.output;
      async function* messages() {
        try {
          for await (const event of events) {
            if (event.type === "message") {
              yield fromBinary(output, event.data);
              continue;
            }
            if (inputError !== undefined) {
              throw ConnectError.from(inputError);
            }
            if (event.error) {
              throw new ConnectError(event.error.message, event.error.code);
            }
            return;
          }
        } finally {
          disposed = true;
          events.close();
          if (!completed) {
            window.desktop.daemon.streamCancel(id);
          }
          void inputIterator.return?.();
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
