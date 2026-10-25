// Bundles the extension into one CommonJS file, `build/extension.js`.
//
// The SSR build targets Node; every dependency (vscode-languageclient, ...) is
// bundled, so the VSIX needs no `node_modules`. Only `vscode`, provided by the
// editor, and the Node built-ins stay external.
//
// `vite build --mode development` (used by `.vscode/launch.json`) keeps the
// code readable with a source map for debugging; the default production mode
// minifies and emits no source map.
import * as fs from "node:fs";
import { builtinModules } from "node:module";
import * as path from "node:path";

import { defineConfig } from "vite";

export default defineConfig(({ mode }) => {
  const debug = mode === "development";
  const outDir = path.join(import.meta.dirname, "../build");
  return {
    root: import.meta.dirname,
    plugins: [
      {
        // `build` also holds the server binary and the VSIX, so it is not
        // emptied; drop the source map of a previous debug build instead.
        name: "remove-stale-sourcemap",
        buildStart() {
          if (!debug) fs.rmSync(path.join(outDir, "extension.js.map"), { force: true });
        },
      },
    ],
    ssr: {
      noExternal: true,
      target: "node",
    },
    build: {
      ssr: "src/extension.ts",
      outDir,
      emptyOutDir: false,
      sourcemap: debug,
      minify: !debug,
      target: "node20",
      rollupOptions: {
        external: ["vscode", ...builtinModules, ...builtinModules.map((m) => `node:${m}`)],
        output: {
          format: "cjs",
          entryFileNames: "extension.js",
        },
      },
    },
  };
});
