import { safeStorage } from "electron";

const ENCRYPTED_VALUE_PREFIX = "safe-storage:";

export function encodeSecureString(value: string): string {
  if (!safeStorage.isEncryptionAvailable()) {
    if (process.platform === "win32") {
      throw new Error("Windows credential encryption is unavailable");
    }
    return value;
  }
  return `${ENCRYPTED_VALUE_PREFIX}${safeStorage.encryptString(value).toString("base64")}`;
}

export function decodeSecureString(value: string): string {
  if (!value.startsWith(ENCRYPTED_VALUE_PREFIX)) {
    if (process.platform === "win32") {
      throw new Error("unencrypted Windows credential storage");
    }
    return value;
  }
  if (!safeStorage.isEncryptionAvailable()) {
    throw new Error("credential encryption is unavailable");
  }
  return safeStorage.decryptString(
    Buffer.from(value.slice(ENCRYPTED_VALUE_PREFIX.length), "base64"),
  );
}
