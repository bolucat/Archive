import { defineConfig, minimal2023Preset } from "@vite-pwa/assets-generator/config";

export default defineConfig({
  preset: {
    ...minimal2023Preset,
    maskable: {
      ...minimal2023Preset.maskable,
      resizeOptions: { fit: "contain", background: "#eceff1" },
    },
    apple: {
      ...minimal2023Preset.apple,
      resizeOptions: { fit: "contain", background: "#eceff1" },
    },
  },
  images: ["public/favicon.svg"],
});
