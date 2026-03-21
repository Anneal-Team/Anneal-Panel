import type { Config } from "tailwindcss";

export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "#f3efe6",
        foreground: "#132226",
        card: "#fbf7ef",
        border: "#d9ccb8",
        accent: "#174b4f",
        muted: "#e6dccd",
        danger: "#8d3d29",
      },
      fontFamily: {
        sans: ["'IBM Plex Sans'", "sans-serif"],
      },
      boxShadow: {
        panel: "0 18px 50px rgba(19, 34, 38, 0.12)",
      },
    },
  },
  plugins: [],
} satisfies Config;
