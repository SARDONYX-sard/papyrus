// The tasks of the repository, used locally (`npm run ...`) and by the CI
// workflows.
//
// usage: node tasks.cjs <task> [options]; `node tasks.cjs help` lists them.
const fs = require("fs");
const path = require("path");
const { execFileSync, execSync } = require("child_process");

/** Rust target triples and the matching VS Code platforms. */
const PLATFORMS = {
  "x86_64-pc-windows-msvc": "win32-x64",
  "aarch64-pc-windows-msvc": "win32-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-apple-darwin": "darwin-arm64",
};

/** The value after `name` in `args`. */
const option = (args, name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};

const run = (cmd) => {
  console.log(`> ${cmd}`);
  execSync(cmd, { cwd: __dirname, stdio: "inherit" });
};

/** Runs `git` with `args`, without a shell: messages need no quoting. */
const git = (...args) => {
  console.log(`> git ${args.join(" ")}`);
  return execFileSync("git", args, { cwd: __dirname, encoding: "utf8", stdio: ["inherit", "pipe", "inherit"] }).trim();
};

/**
 * Copies the language server next to the compiled extension in build/, and
 * optionally packages a platform specific VSIX.
 */
function build(args) {
  const target = option(args, "--target");
  const profile = option(args, "--profile") ?? "release";
  const windows = target ? target.includes("windows") : process.platform === "win32";
  const exe = windows ? "papyrus_analyzer.exe" : "papyrus_analyzer";
  const src = option(args, "--binary") ?? path.join(__dirname, "target", ...(target ? [target] : []), profile, exe);

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

  if (!args.includes("--vsix")) return;
  run("npm run check");
  run("npm run compile");
  const platform = target ? PLATFORMS[target] : undefined;
  if (target && !platform) throw new Error(`no VS Code platform for target ${target}`);
  const out = `build/papyrus-analyzer${platform ? `-${platform}` : ""}.vsix`;
  // Vite bundles the dependencies into build/extension.cjs. vsce only looks for
  // one `LICENSE[.md|.txt]` file; LICENSE-MIT and LICENSE-APACHE are packaged
  // through .vscodeignore, so its check (and its y/N prompt) is skipped.
  // vsce comes from devDependencies, at the version of package-lock.json.
  run(`npx vsce package --no-dependencies --skip-license ${platform ? `--target ${platform}` : ""} --out ${out}`);
  console.log(`packaged ${out}`);
}

/**
 * Tags HEAD as `<version>-dev.<sha>` (signed, annotated, replacing a tag of
 * the same name) and pushes the tag: build.yaml then attaches the VSIX and
 * the archives to its GitHub release.
 */
function tag(args) {
  const { version } = JSON.parse(fs.readFileSync(path.join(__dirname, "package.json"), "utf8"));
  const sha = git("rev-parse", "--short=7", "HEAD");
  const name = `${version}-dev.${sha}`;
  const message = option(args, "-m") ?? git("log", "-1", "--format=%s");
  git("tag", name, "-m", message, "-s", "--annotate", "-f");
  if (args.includes("--no-push")) {
    console.log(`tagged ${name}`);
    return;
  }
  // Only this tag; forced like the tag itself, so that a retag replaces it.
  git("push", "--force", "origin", `refs/tags/${name}`);
  console.log(`pushed ${name}`);
}

/** The manifest of the server crate, whose version is the release version. */
const CRATE_MANIFEST = path.join(__dirname, "crates", "papyrus_analyzer", "Cargo.toml");

/** `[major, minor, patch]` of a `x.y.z` version. */
const parseVersion = (v) => {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(v);
  if (!m) throw new Error(`not a x.y.z version: ${v}`);
  return m.slice(1).map(Number);
};
const compareVersions = (a, b) => {
  const [x, y] = [parseVersion(a), parseVersion(b)];
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
};

/**
 * Bumps the version of the extension (package.json, package-lock.json) and
 * of the server crate (crates/papyrus_analyzer/Cargo.toml) to the same new
 * version, updates only the workspace crates in Cargo.lock, then commits and
 * tags the new version; `current` only tags the current version. With
 * `--sign`, the commit (`-S`) and the tag (`-s`) are signed. Nothing is pushed.
 */
function release(args) {
  const level = args[0];
  if (!["major", "minor", "patch", "current"].includes(level)) {
    throw new Error("usage: node tasks.cjs release <major|minor|patch|current> [--sign]");
  }
  const sign = args.includes("--sign");
  const npmVersion = JSON.parse(fs.readFileSync(path.join(__dirname, "package.json"), "utf8")).version;
  const manifest = fs.readFileSync(CRATE_MANIFEST, "utf8");
  // `version    = "x.y.z"`: the alignment before the value is kept.
  const crateLine = /^(version\s*=\s*)"([^"]+)"/m;
  const crateVersion = crateLine.exec(manifest)?.[2];
  if (!crateVersion) throw new Error(`no version in ${CRATE_MANIFEST}`);

  // `git tag` signs with `-s` (`-S` is the switch of `git commit` only).
  const tagFlags = sign ? ["-s"] : ["--annotate"];
  if (level === "current") {
    // The VSIX and the server of the release must say the same version.
    if (npmVersion !== crateVersion) {
      throw new Error(
        `package.json ${npmVersion} and the crate ${crateVersion} differ: release a new version (major, minor or patch)`,
      );
    }
    if (git("tag", "--list", npmVersion)) throw new Error(`the tag ${npmVersion} exists already`);
    git("tag", ...tagFlags, "-m", `build: release ${npmVersion}`, npmVersion);
    console.log(`tagged ${npmVersion}; push with: git push origin ${npmVersion}`);
    return;
  }

  const files = ["package.json", "package-lock.json", "crates/papyrus_analyzer/Cargo.toml", "Cargo.lock"];
  // Only these files are committed: they must not hold other changes.
  const dirty = git("status", "--porcelain", "--", ...files);
  if (dirty) throw new Error(`commit or stash the changes of these files first:\n${dirty}`);
  // The two are released together; the newer one counts.
  const old = compareVersions(npmVersion, crateVersion) >= 0 ? npmVersion : crateVersion;
  const [major, minor, patch] = parseVersion(old);
  const next = {
    major: `${major + 1}.0.0`,
    minor: `${major}.${minor + 1}.0`,
    patch: `${major}.${minor}.${patch + 1}`,
  }[level];
  if (git("tag", "--list", next)) throw new Error(`the tag ${next} exists already`);
  console.log(`${old} -> ${next} (package.json ${npmVersion}, crate ${crateVersion})`);

  // package.json and package-lock.json, without npm's own commit and tag.
  run(`npm version ${next} --no-git-tag-version --allow-same-version`);
  fs.writeFileSync(CRATE_MANIFEST, manifest.replace(crateLine, (_, key) => `${key}"${next}"`));
  // Only the workspace members: the versions of the dependencies stay.
  run("cargo update --workspace");

  git("add", "--", ...files);
  const message = `build: bump up to ${old} -> ${next}`;
  git("commit", ...(sign ? ["-S"] : []), "-m", message);
  git("tag", ...tagFlags, "-m", message, next);
  console.log(`committed and tagged ${next}; push with: git push origin HEAD ${next}`);
}

const TASKS = {
  release: {
    usage: "<major|minor|patch|current> [--sign]",
    help: `major, minor, patch: bumps the version of package.json and of the papyrus_analyzer
crate (from the newer of the two, so that they agree), updates the workspace
crates in Cargo.lock, and makes a commit "build: bump up to <old> -> <new>" and
an annotated tag <new> with the same message.
current: only tags HEAD with the current version (package.json and the crate
must agree).
Nothing is pushed.
  --sign  sign the commit (git commit -S) and the tag (git tag -s)`,
    run: release,
  },
  build: {
    usage: "[--vsix] [--target <rust triple>] [--profile <name>] [--binary <path>]",
    help: `Copies the server next to the extension in build/.
  --target   the Rust target the server was built for, e.g. x86_64-pc-windows-msvc
             (default: the host build in target/<profile>/)
  --profile  the Cargo profile (default: release)
  --binary   the server binary to copy, instead of looking in target/
  --vsix     also compile the extension and package build/papyrus-analyzer-<platform>.vsix`,
    run: build,
  },
  tag: {
    usage: "[-m <message>] [--no-push]",
    help: `Tags HEAD as <package.json version>-dev.<short sha> (signed, annotated, forced)
and pushes it to origin, which releases it (see .github/workflows/build.yaml).
  -m         the tag message (default: the subject of the last commit)
  --no-push  only create the tag`,
    run: tag,
  },
  help: {
    usage: "[<task>]",
    help: "Shows the tasks, or the options of one.",
    run: (args) => help(args[0]),
  },
};

function help(name) {
  const tasks = name ? { [name]: TASKS[name] } : TASKS;
  if (name && !TASKS[name]) throw new Error(`unknown task ${name}`);
  console.log("usage: node tasks.cjs <task> [options]\n");
  for (const [task, { usage, help }] of Object.entries(tasks)) {
    console.log(`${task} ${usage}\n  ${help.replaceAll("\n", "\n  ")}\n`);
  }
}

const [name, ...args] = process.argv.slice(2);
const task = TASKS[name ?? "help"];
if (!task) {
  console.error(`unknown task ${name}\n`);
  help();
  process.exit(1);
}
Promise.resolve()
  .then(() => task.run(args))
  .catch((e) => {
    console.error(e instanceof Error ? e.message : e);
    process.exit(1);
  });
