// Copies the language server next to the compiled extension in build/, and
// optionally packages a platform specific VSIX.
//
// usage: node build.cjs [--vsix] [--target <rust triple>] [--profile <name>] [--binary <path>]
//
//   --target   the Rust target the server was built for, e.g. x86_64-pc-windows-msvc
//              (default: the host build in target/<profile>/)
//   --profile  the Cargo profile (default: release)
//   --binary   the server binary to copy, instead of looking in target/
//   --vsix     also compile the extension and package build/papyrus-analyzer-<platform>.vsix
//
// The same script is used locally (`npm run release`) and by the CI workflows.
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const args = process.argv.slice(2);
const option = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const target = option("--target");
const profile = option("--profile") ?? "release";
const vsix = args.includes("--vsix");

/** Rust target triples and the matching VS Code platforms. */
const PLATFORMS = {
  "x86_64-pc-windows-msvc": "win32-x64",
  "aarch64-pc-windows-msvc": "win32-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-apple-darwin": "darwin-arm64",
};

const windows = target ? target.includes("windows") : process.platform === "win32";
const exe = windows ? "papyrus_analyzer.exe" : "papyrus_analyzer";
const src =
  option("--binary") ?? path.join(__dirname, "target", ...(target ? [target] : []), profile, exe);

const buildDir = path.join(__dirname, "build");
fs.mkdirSync(buildDir, { recursive: true });
// Only one server binary per package: remove the one of another platform.
for (const old of ["papyrus_analyzer", "papyrus_analyzer.exe"]) {
  if (old !== exe) fs.rmSync(path.join(buildDir, old), { force: true });
}
const dst = path.join(buildDir, exe);
fs.copyFileSync(src, dst);
if (!windows) fs.chmodSync(dst, 0o755);
console.log(`copied ${src} -> ${dst}`);

if (vsix) {
  const run = (cmd) => {
    console.log(`> ${cmd}`);
    execSync(cmd, { cwd: __dirname, stdio: "inherit" });
  };
  run("npm run check");
  run("npm run compile");
  const platform = target ? PLATFORMS[target] : undefined;
  if (target && !platform) throw new Error(`no VS Code platform for target ${target}`);
  const out = `build/papyrus-analyzer${platform ? `-${platform}` : ""}.vsix`;
  // Vite bundles the dependencies into build/extension.cjs. vsce only looks for
  // one `LICENSE[.md|.txt]` file; LICENSE-MIT and LICENSE-APACHE are packaged
  // through .vscodeignore, so its check (and its y/N prompt) is skipped.
  // vsce comes from devDependencies, at the version of package-lock.json.
  run(
    `npx vsce package --no-dependencies --skip-license ${platform ? `--target ${platform}` : ""} --out ${out}`,
  );
  console.log(`packaged ${out}`);
}
