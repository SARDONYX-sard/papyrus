// Copies the release build of the language server next to the compiled extension.
const fs = require("fs");
const path = require("path");

const exe = process.platform === "win32" ? "papyrus_lsp.exe" : "papyrus_lsp";
const src = path.join(__dirname, "target", "release", exe);

const distDir = path.join(__dirname, "dist");
const dst = path.join(distDir, exe);

fs.mkdirSync(distDir, { recursive: true });
fs.copyFileSync(src, dst);
console.log(`copied ${src} -> ${dst}`);
