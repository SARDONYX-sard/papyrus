const fs = require("fs");
const path = require("path");

const src = path.join(__dirname, "target", "release", "papyrus_lsp.exe");

const distDir = path.join(__dirname, "dist");
const dst = path.join(distDir, "papyrus_lsp.exe");

fs.mkdirSync(distDir, { recursive: true });
fs.copyFileSync(src, dst);
