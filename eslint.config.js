import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  { ignores: ["dist", "src-tauri/target", "node_modules"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Ungenutzte Variablen sind hier ein Fehler, nicht bloß ein Hinweis —
      // sie sind der beste Indikator für Reste aus Umbauten. `_`-Präfix
      // markiert bewusst Ungenutztes.
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // Der Fehler-Resolver nimmt bewusst `unknown` entgegen; `any` wollen wir nicht.
      "@typescript-eslint/no-explicit-any": "error",
    },
  }
);
