/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: {
          900: "#0a0d14",
          800: "#0f1623",
          700: "#111b2e",
          500: "#1b2a45",
          300: "#2c3f63",
        },
        neon: {
          400: "#6ee7ff",
          500: "#38bdf8",
          600: "#0ea5e9",
        },
        lava: {
          400: "#ff7aa8",
          500: "#ff4f8b",
        },
      },
      boxShadow: {
        glow: "0 0 35px rgba(56, 189, 248, 0.45)",
      },
      animation: {
        scan: "scan 2.2s linear infinite",
        float: "float 6s ease-in-out infinite",
      },
      keyframes: {
        scan: {
          "0%": { transform: "translateY(-100%)" },
          "100%": { transform: "translateY(100%)" },
        },
        float: {
          "0%, 100%": { transform: "translateY(0px)" },
          "50%": { transform: "translateY(-10px)" },
        },
      },
    },
  },
  plugins: [],
};
