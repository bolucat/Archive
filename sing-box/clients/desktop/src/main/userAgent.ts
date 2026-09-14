import { preferredLocale } from "./locale";

const applicationName = process.platform === "linux" ? "SFL" : "SFW";

export function userAgent(): string {
  return `${applicationName} (sing-box ${__APP_VERSION__}; language ${preferredLocale().replaceAll("-", "_")})`;
}
