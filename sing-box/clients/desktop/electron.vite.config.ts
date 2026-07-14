import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import type { Plugin } from "vite";

import { readApplicationVersion } from "./scripts/version";

function contentSecurityPolicy(): Plugin {
  let development = false;
  return {
    name: "content-security-policy",
    configResolved(config) {
      development = config.command === "serve";
    },
    transformIndexHtml(html) {
      const inlineScriptSources = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(
        ([, body]) => `'sha256-${createHash("sha256").update(body).digest("base64")}'`,
      );
      const scriptSource = development
        ? "'self' 'unsafe-inline'"
        : ["'self'", ...inlineScriptSources].join(" ");
      return [
        {
          tag: "meta",
          attrs: {
            "http-equiv": "Content-Security-Policy",
            content: [
              "default-src 'self'",
              `script-src ${scriptSource}`,
              "style-src 'self' 'unsafe-inline'",
              "img-src 'self' data:",
              "font-src 'self' data:",
              "connect-src 'self' http: https: ws: wss:",
            ].join("; "),
          },
          injectTo: "head-prepend",
        },
      ];
    },
  };
}

export default defineConfig({
  main: {
    define: {
      __APP_VERSION__: JSON.stringify(readApplicationVersion()),
    },
  },
  preload: {
    build: {
      rollupOptions: {
        output: {
          format: "cjs",
        },
      },
    },
  },
  renderer: {
    build: {
      rollupOptions: {
        input: {
          index: resolve(import.meta.dirname, "src/renderer/index.html"),
          tray: resolve(import.meta.dirname, "src/renderer/tray.html"),
        },
      },
    },
    css: {
      modules: {
        localsConvention: "camelCaseOnly",
      },
    },
    resolve: {
      alias: {
        "@dashboard": resolve(import.meta.dirname, "dashboard/src"),
        "@shared": resolve(import.meta.dirname, "src/shared"),
      },
      dedupe: [
        "react",
        "react-dom",
        "@bufbuild/protobuf",
        "@connectrpc/connect",
        "@connectrpc/connect-web",
      ],
    },
    plugins: [react(), contentSecurityPolicy()],
  },
});
