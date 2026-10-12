/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg0: "#0c0d10",
        bg1: "#14161b",
        bg2: "#1b1e25",
        bg3: "#242833",
        line: "#2e3440",
        ink: "#e8eaef",
        muted: "#a7b0bd",
        faint: "#7f8a99",
        accent: "#3b82f6",
      },
      fontFamily: {
        sans: ['"Segoe UI"', "Inter", "Liberation Sans", "system-ui", "sans-serif"],
      },
      borderRadius: {
        sm: "3px",
      },
      fontSize: {
        ui: ["12px", "16px"],
      },
    },
  },
  plugins: [],
};
