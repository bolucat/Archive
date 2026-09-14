import type { Interceptor } from "@connectrpc/connect";
import { app } from "electron";

import { DESKTOP_LANGUAGES } from "../shared/translations";
import { preferenceSnapshot } from "./database";

const desktopLanguages = new Set<string>(DESKTOP_LANGUAGES);

export function preferredLocale(): string {
  const language = preferenceSnapshot(["language"]).language;
  if (typeof language === "string" && desktopLanguages.has(language)) {
    return language;
  }
  return app.getLocale();
}

export const localeInterceptor: Interceptor = (next) => async (request) => {
  if (!request.header.has("accept-language")) {
    request.header.set("accept-language", preferredLocale());
  }
  return next(request);
};
