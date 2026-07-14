import type { Client, Transport } from "@connectrpc/connect";
import { createClient } from "@connectrpc/connect";
import { createGrpcTransport } from "@connectrpc/connect-node";
import { spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import * as net from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ApplicationService } from "../shared/gen/experimental/boxdd/desktop_service_pb";
import { daemonBinaryPath } from "./repair";

// A plain stdio transport (HTTP/2 over the child's standard streams) was
// rejected: Node wraps a non-socket duplex in JSStreamSocket, which delivers
// a whole response in one tick, and the http2 stream async iterator then
// misreports "Premature close" when connect-node attaches it after the
// response event.

function workerEndpoint(): string {
  if (process.platform === "win32") {
    return `\\\\.\\pipe\\sing-box-worker.${randomUUID()}`;
  }
  return join(tmpdir(), `sing-box-worker.${randomUUID().slice(0, 8)}.sock`);
}

interface WorkerProcess {
  child: ChildProcess;
  applicationTransport: Transport;
  daemonTransport: Transport | null;
}

let currentWorker: Promise<WorkerProcess> | null = null;
const WORKER_READY_TIMEOUT_MILLISECONDS = 10_000;

function spawnWorker(): Promise<WorkerProcess> {
  return new Promise((resolve, reject) => {
    const endpoint = workerEndpoint();
    const daemonRelayEndpoint = process.platform === "win32" ? workerEndpoint() : null;
    const commandArguments = ["worker", "--socket", endpoint, "--parent-pid", String(process.pid)];
    if (daemonRelayEndpoint !== null) {
      commandArguments.push("--daemon-relay-socket", daemonRelayEndpoint);
    }
    const child = spawn(daemonBinaryPath(), commandArguments, {
      stdio: ["pipe", "pipe", "inherit"],
      windowsHide: true,
    });
    let settled = false;
    const fail = (error: Error) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      child.kill();
      reject(error);
    };
    const timer = setTimeout(
      () => fail(new Error("worker did not become ready in time")),
      WORKER_READY_TIMEOUT_MILLISECONDS,
    );
    child.once("error", (error) => fail(error));
    child.once("exit", () => fail(new Error("worker exited before becoming ready")));
    child.stdout?.setEncoding("utf-8");
    let readinessOutput = "";
    const handleReadinessOutput = (data: string) => {
      readinessOutput += data;
      const lineEnd = readinessOutput.indexOf("\n");
      if (lineEnd === -1 && readinessOutput.length <= 64) {
        return;
      }
      child.stdout?.off("data", handleReadinessOutput);
      if (lineEnd === -1 || readinessOutput.slice(0, lineEnd).trim() !== "READY") {
        fail(new Error("worker returned an invalid readiness response"));
        return;
      }
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve({
        child,
        applicationTransport: createGrpcTransport({
          baseUrl: "http://sing-box-worker",
          nodeOptions: {
            createConnection: () => net.connect({ path: endpoint }),
          },
        }),
        daemonTransport:
          daemonRelayEndpoint === null
            ? null
            : createGrpcTransport({
                baseUrl: "http://sing-box",
                nodeOptions: {
                  createConnection: () => net.connect({ path: daemonRelayEndpoint }),
                },
              }),
      });
    };
    child.stdout?.on("data", handleReadinessOutput);
  });
}

async function workerConnection(): Promise<WorkerProcess> {
  if (currentWorker === null) {
    const workerPromise = spawnWorker();
    currentWorker = workerPromise;
    const release = () => {
      if (currentWorker === workerPromise) {
        currentWorker = null;
      }
    };
    workerPromise.then(({ child }) => child.once("exit", release), release);
  }
  return currentWorker;
}

export const workerTransport: Transport = {
  unary: async (method, signal, timeoutMs, header, message, contextValues) =>
    (await workerConnection()).applicationTransport.unary(
      method,
      signal,
      timeoutMs,
      header,
      message,
      contextValues,
    ),
  stream: async (method, signal, timeoutMs, header, input, contextValues) =>
    (await workerConnection()).applicationTransport.stream(
      method,
      signal,
      timeoutMs,
      header,
      input,
      contextValues,
    ),
};

async function daemonWorkerConnection(): Promise<Transport> {
  const transport = (await workerConnection()).daemonTransport;
  if (transport === null) {
    throw new Error("daemon relay is unavailable on this platform");
  }
  return transport;
}

export const daemonWorkerTransport: Transport = {
  unary: async (method, signal, timeoutMs, header, message, contextValues) =>
    (await daemonWorkerConnection()).unary(
      method,
      signal,
      timeoutMs,
      header,
      message,
      contextValues,
    ),
  stream: async (method, signal, timeoutMs, header, input, contextValues) =>
    (await daemonWorkerConnection()).stream(
      method,
      signal,
      timeoutMs,
      header,
      input,
      contextValues,
    ),
};

export const applicationService: Client<typeof ApplicationService> = createClient(
  ApplicationService,
  workerTransport,
);
