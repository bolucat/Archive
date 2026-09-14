import type {
  DescMessage,
  DescMethodBiDiStreaming,
  MessageInitShape,
  MessageShape,
} from "@bufbuild/protobuf";
import { ConnectError } from "@connectrpc/connect";
import type { Transport } from "@connectrpc/connect";
import { createWritableIterable } from "@connectrpc/connect/protocol";

import type { Server } from "./config";
import { GrpcWebSocketStream, type GrpcStatus } from "./websocket";

export type { GrpcStatus };

export interface BidirectionalStream<I extends DescMessage> {
  send(message: MessageInitShape<I>): void;
  close(): void;
}

export interface BidirectionalStreamHandlers<O extends DescMessage> {
  onMessage(message: MessageShape<O>): void;
  onEnd(status: GrpcStatus | null, error?: string): void;
}

export function openBidirectionalStream<I extends DescMessage, O extends DescMessage>(
  config: Server,
  language: string,
  method: DescMethodBiDiStreaming<I, O>,
  handlers: BidirectionalStreamHandlers<O>,
  transport?: Transport,
): BidirectionalStream<I> {
  if (transport !== undefined) {
    return new TransportBidirectionalStream(transport, method, handlers);
  }
  return new GrpcWebSocketStream({
    config,
    language,
    service: method.parent.typeName,
    method: method.name,
    requestSchema: method.input,
    responseSchema: method.output,
    onMessage: handlers.onMessage,
    onEnd: handlers.onEnd,
  });
}

class TransportBidirectionalStream<I extends DescMessage, O extends DescMessage>
  implements BidirectionalStream<I>
{
  private readonly controller = new AbortController();
  private readonly input = createWritableIterable<MessageInitShape<I>>();
  private ended = false;

  constructor(
    private transport: Transport,
    private method: DescMethodBiDiStreaming<I, O>,
    private handlers: BidirectionalStreamHandlers<O>,
  ) {
    void this.run();
  }

  send(message: MessageInitShape<I>) {
    if (this.ended) {
      return;
    }
    void this.input.write(message).catch((error: unknown) => {
      const connectError = ConnectError.from(error);
      this.finish({ code: connectError.code, message: connectError.rawMessage });
    });
  }

  close() {
    if (this.ended) {
      return;
    }
    this.ended = true;
    this.input.close();
    this.controller.abort();
  }

  private async run() {
    try {
      const response = await this.transport.stream(
        this.method,
        this.controller.signal,
        undefined,
        undefined,
        this.input,
      );
      for await (const message of response.message) {
        if (this.ended) {
          return;
        }
        this.handlers.onMessage(message);
      }
      this.finish({ code: 0, message: "" });
    } catch (error) {
      if (this.ended) {
        return;
      }
      const connectError = ConnectError.from(error);
      this.finish({ code: connectError.code, message: connectError.rawMessage });
    }
  }

  private finish(status: GrpcStatus) {
    if (this.ended) {
      return;
    }
    this.ended = true;
    this.input.close();
    this.handlers.onEnd(status);
  }
}
