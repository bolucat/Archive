import path from "node:path";
import AutoImport from "unplugin-auto-import/vite";
import IconsResolver from "unplugin-icons/resolver";
import Icons from "unplugin-icons/vite";
import { defineConfig } from "vite";
import monaco from "vite-plugin-monaco-editor";
import sassDts from "vite-plugin-sass-dts";
import svgr from "vite-plugin-svgr";
import tsconfigPaths from "vite-tsconfig-paths";
import generouted from "@generouted/react-router/plugin";
// import react from "@vitejs/plugin-react";
import react from "@vitejs/plugin-react-swc";

const devtools = () => {
  return {
    name: "react-devtools",
    transformIndexHtml(html) {
      return html.replace(
        /<\/head>/,
        `<script src="http://localhost:8097"></script></head>`,
      );
    },
  };
};

const IS_NIGHTLY = process.env.NIGHTLY?.toLowerCase() === "true";

// https://vitejs.dev/config/
export default defineConfig(({ command }) => {
  const isDev = command === "serve";

  return {
    // root: "/",
    server: { port: 3000 },
    css: {
      preprocessorOptions: {
        scss: {
          importer(...args) {
            if (args[0] !== "@/styles") {
              return;
            }

            return {
              file: `${path.resolve(__dirname, "./src/assets/styles")}`,
            };
          },
        },
      },
    },
    plugins: [
      tsconfigPaths(),
      svgr(),
      react({
        // babel: {
        //   plugins: ["@emotion/babel-plugin"],
        // },
      }),
      AutoImport({
        resolvers: [
          IconsResolver({
            prefix: "Icon",
            extension: "jsx",
          }),
        ],
      }),
      Icons({
        compiler: "jsx", // or 'solid'
      }),
      generouted(),
      sassDts({ esmExport: true }),
      monaco({
        languageWorkers: ["editorWorkerService", "typescript"],
        customWorkers: [
          {
            label: "yaml",
            entry: "monaco-yaml/yaml.worker",
          },
        ],
      }),
      isDev && devtools(),
    ],
    resolve: {
      alias: {
        "@repo": path.resolve("../../"),
        "@nyanpasu/ui": path.resolve("../ui/src"),
        "@nyanpasu/interface": path.resolve("../interface/src"),
      },
    },
    optimizeDeps: {
      entries: ["./src/pages/**/*.tsx", "./src/main.tsx"],
      include: ["@emotion/styled"],
    },
    esbuild: {
      drop: isDev ? undefined : ["debugger"],
      pure: isDev || IS_NIGHTLY ? [] : ["console.log"],
    },
    build: {
      outDir: "dist",
      emptyOutDir: true,
      sourcemap: isDev || IS_NIGHTLY ? "inline" : false,
    },
    define: {
      OS_PLATFORM: `"${process.platform}"`,
      WIN_PORTABLE: !!process.env.VITE_WIN_PORTABLE,
    },
    html: {},
  };
});
