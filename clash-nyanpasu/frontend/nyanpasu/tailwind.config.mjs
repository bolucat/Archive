import { MUI_BREAKPOINTS } from "@nyanpasu/ui/materialYou/createTheme";

/* eslint-disable @typescript-eslint/no-var-requires */
/* eslint-disable @typescript-eslint/no-require-imports */
const plugin = require("tailwindcss/plugin");

const getMUuiScreen = () => {
  const breakpoints = MUI_BREAKPOINTS.values;

  const result = {};

  for (const key in breakpoints) {
    if (breakpoints.hasOwnProperty(key)) {
      result[key] = `${breakpoints[key]}px`;
    }
  }

  return result;
};

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{tsx,ts}", "../ui/**/*.{tsx,ts}"],
  darkMode: "selector",
  theme: {
    extend: {
      maxHeight: {
        "1/8": "calc(100vh / 8)",
      },
      zIndex: {
        top: 100000,
      },
      animation: {
        marquee: "marquee 4s linear infinite",
      },
      keyframes: {
        marquee: {
          "0%": { transform: "translateX(100%)" },
          "100%": { transform: "translateX(-100%)" },
        },
      },
      colors: {
        scroller: "var(--scroller-color)",
        container: "var(--background-color)",
      },
    },
    screen: getMUuiScreen(),
  },
  plugins: [
    require("tailwindcss-textshadow"),
    plugin(({ addBase }) => {
      addBase({
        ".scrollbar-hidden::-webkit-scrollbar": {
          width: "0px",
        },
      });
    }),
  ],
};
