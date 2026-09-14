import type { DesktopHost } from "../app/desktop";

type SharingHost = Pick<DesktopHost, "platform" | "application">;

function filePart(data: Uint8Array | string | Blob): BlobPart {
  if (typeof data === "string" || data instanceof Blob) {
    return data;
  }
  const copy = new Uint8Array(data.byteLength);
  copy.set(data);
  return copy.buffer;
}

function canUseWebShareFiles(): boolean {
  if (typeof navigator.share !== "function" || typeof navigator.canShare !== "function") {
    return false;
  }
  try {
    return navigator.canShare({ files: [new File([], "share.txt", { type: "text/plain" })] });
  } catch {
    return false;
  }
}

export function canShare(host: SharingHost | null): boolean {
  return host?.platform === "win32" || typeof navigator.share === "function";
}

export function canShareFiles(host: SharingHost | null): boolean {
  return host?.platform === "win32" || canUseWebShareFiles();
}

export async function shareFile(
  host: SharingHost | null,
  name: string,
  data: Uint8Array | string | Blob,
  type: string,
): Promise<void> {
  if (host?.platform === "win32") {
    await host.application.shareFile(
      name,
      data instanceof Blob ? new Uint8Array(await data.arrayBuffer()) : data,
    );
    return;
  }
  const file = new File([filePart(data)], name, { type });
  if (typeof navigator.share === "function") {
    if (navigator.canShare?.({ files: [file] })) {
      await navigator.share({ files: [file] });
      return;
    }
    if (typeof data === "string") {
      await navigator.share({ text: data });
      return;
    }
  }
  throw new Error("File sharing is not supported");
}

export function shareError(error: unknown): unknown | null {
  if (error instanceof DOMException && error.name === "AbortError") {
    return null;
  }
  return error;
}
