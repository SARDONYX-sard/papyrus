import * as cp from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  RequestType,
  RequestType0,
  ServerOptions,
  State,
} from "vscode-languageclient/node";

/** Result of the `papyrus/status` and `papyrus/restart` requests. */
interface ServerStatus {
  files: number;
  openFiles: number;
  indexing: boolean;
  generation: number;
  roots: string[];
  importDirectories: string[];
  health?: "ok" | "limited" | "warning" | "error";
  problems?: string[];
  manifest?: string | null;
  manifestError?: string | null;
  sources?: string[];
  flagsFile?: string | null;
  variables?: Record<string, string>;
}

/** Result of `papyrus/compileCommands`. */
interface CompileCommands {
  manifest: string;
  output: string;
  commands: { program: string; args: string[]; cwd: string; output: string; commandLine: string }[];
}

/**
 * `papyrus/restart`: the server drops its indexes, applies `settings`,
 * re-indexes the workspace and republishes diagnostics. Implemented on the
 * server side because an activated extension cannot restart itself.
 */
const RestartRequest = new RequestType<{ settings?: unknown }, ServerStatus, void>("papyrus/restart");
const StatusRequest = new RequestType0<ServerStatus, void>("papyrus/status");
const SyntaxTreeRequest = new RequestType<{ textDocument: { uri: string } }, string, void>(
  "papyrus/syntaxTree",
);
const CompileCommandsRequest = new RequestType<
  { textDocument?: { uri: string }; files?: { uri: string }[]; all: boolean },
  CompileCommands,
  void
>("papyrus/compileCommands");
const ManifestTemplateRequest = new RequestType0<string, void>("papyrus/manifestTemplate");

/** Task definition of `papyrus` tasks, see `taskDefinitions` in package.json. */
interface PapyrusTaskDefinition extends vscode.TaskDefinition {
  all?: boolean;
  files?: string[];
}

let client: LanguageClient | undefined;
let statusItem: vscode.StatusBarItem;
/** `$(run-all) Compile…`: compiles the scripts picked with a quick pick. */
let compileItem: vscode.StatusBarItem;
let output: vscode.LogOutputChannel;
let extensionContext: vscode.ExtensionContext;
/** Key of the scripts picked last, in the workspace state. */
const SELECTION_KEY = "papyrus.compileSelection";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  output = vscode.window.createOutputChannel("Papyrus Analyzer", { log: true });
  statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  statusItem.command = "papyrus.showStatus";
  compileItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, -1);
  compileItem.command = "papyrus.compileSelected";
  compileItem.text = "$(run-all) Compile…";
  compileItem.tooltip = "Papyrus: pick the scripts to compile together";
  extensionContext = context;
  context.subscriptions.push(output, statusItem, compileItem);

  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restartServer", restartServer),
    vscode.commands.registerCommand("papyrus.showStatus", showStatus),
    vscode.commands.registerCommand("papyrus.showSyntaxTree", showSyntaxTree),
    vscode.commands.registerCommand("papyrus.compile", compileCommand),
    vscode.commands.registerCommand("papyrus.compileAll", () => runCompile({ type: "papyrus", all: true })),
    vscode.commands.registerCommand("papyrus.compileSelected", compileSelected),
    vscode.commands.registerCommand("papyrus.createManifest", createManifest),
    vscode.tasks.registerTaskProvider("papyrus", {
      provideTasks: () => (hasManifest ? [compileTask({ type: "papyrus", all: true })] : []),
      resolveTask: (task) => compileTask(task.definition as PapyrusTaskDefinition),
    }),
  );

  await startClient(context);
}

export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}

/** The `papyrus` settings section, sent to the server. */
function settings(): Record<string, unknown> {
  const config = vscode.workspace.getConfiguration("papyrus");
  return {
    importDirectories: config.get<string[]>("importDirectories", []).map(expandPath),
    semanticDiagnostics: config.get<boolean>("semanticDiagnostics", true),
    flagsFile: expandPath(config.get<string>("flagsFile", "")) || null,
    var: config.get<Record<string, string>>("var", {}),
    inlayHints: { parameterNames: config.get<boolean>("inlayHints.parameterNames", true) },
  };
}

/** Expands `~` and `${workspaceFolder}` in a configured path. */
function expandPath(p: string): string {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
  const home = process.env.HOME ?? process.env.USERPROFILE ?? "";
  return p.replace("${workspaceFolder}", folder).replace(/^~(?=$|[\\/])/, home);
}

function serverPath(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace.getConfiguration("papyrus").get<string>("server.path");
  if (configured) {
    return expandPath(configured);
  }
  const exe = process.platform === "win32" ? "papyrus_analyzer.exe" : "papyrus_analyzer";
  return context.asAbsolutePath(path.join("dist", exe));
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  const command = serverPath(context);
  if (!fs.existsSync(command)) {
    void vscode.window.showErrorMessage(
      `Papyrus language server not found: ${command}. Build it with \`cargo build --release\` or set \`papyrus.server.path\`.`,
    );
    return;
  }

  const args = vscode.workspace.getConfiguration("papyrus").get<string[]>("server.extraArgs", []);
  const serverOptions: ServerOptions = {
    run: { command, args },
    debug: { command, args, options: { env: { ...process.env, RUST_LOG: "debug" } } },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "papyrus" },
      { scheme: "file", language: "papyrus-flags" },
      { scheme: "file", pattern: "**/papyrus.toml" },
      { scheme: "file", pattern: "**/papyrus.local.toml" },
    ],
    initializationOptions: settings(),
    outputChannel: output,
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/{*.psc,*.flg,papyrus.toml,papyrus.local.toml}"),
    },
  };

  client = new LanguageClient("papyrus", "Papyrus Analyzer", serverOptions, clientOptions);
  context.subscriptions.push(
    client.onDidChangeState((e) => {
      if (e.newState === State.Running) {
        void refreshStatus();
      } else if (e.newState === State.Stopped) {
        statusItem.text = "$(error) Papyrus";
        statusItem.tooltip = "The Papyrus language server stopped. Run `Papyrus: Restart Language Server`.";
        statusItem.show();
      }
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      // The server reloads `papyrus.toml` on save; refresh the compile button.
      if (["papyrus.toml", "papyrus.local.toml"].includes(path.basename(doc.fileName).toLowerCase())) {
        setTimeout(() => void refreshStatus(), 300);
      }
    }),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("papyrus") && client?.isRunning()) {
        void client.sendNotification("workspace/didChangeConfiguration", { settings: settings() });
        setTimeout(() => void refreshStatus(), 500);
      }
    }),
  );

  try {
    await client.start();
  } catch (error) {
    output.appendLine(`Failed to start the Papyrus language server: ${String(error)}`);
    void vscode.window.showErrorMessage("Failed to start the Papyrus language server. See the output for details.");
  }
}

async function restartServer(): Promise<void> {
  if (!client) {
    void vscode.window.showErrorMessage("The Papyrus language server is not running.");
    return;
  }
  if (!client.isRunning()) {
    // The process died: start a new one.
    await client.start();
    return;
  }
  try {
    const status = await client.sendRequest(RestartRequest, { settings: settings() });
    updateStatus(status);
    void vscode.window.setStatusBarMessage("Papyrus language server restarted", 3000);
  } catch (error) {
    output.appendLine(`papyrus/restart failed: ${String(error)}`);
    void vscode.window.showErrorMessage("Failed to restart the Papyrus language server.");
  }
}

async function refreshStatus(): Promise<void> {
  if (!client?.isRunning()) {
    return;
  }
  try {
    updateStatus(await client.sendRequest(StatusRequest));
  } catch {
    // Older servers do not implement `papyrus/status`.
  }
}

let hasManifest = false;

function updateStatus(status: ServerStatus): void {
  hasManifest = !!status.manifest;
  sources = status.sources ?? [];
  void vscode.commands.executeCommand("setContext", "papyrus.hasManifest", hasManifest);
  if (hasManifest) {
    compileItem.show();
  } else {
    compileItem.hide();
  }
  // Red for errors, yellow for warnings, blue without papyrus.toml (limited
  // mode: no compiling), green when everything resolves.
  const health = status.health ?? "ok";
  const icon = { ok: "$(check)", limited: "$(info)", warning: "$(warning)", error: "$(error)" }[health];
  const mode = health === "limited" ? " (limited)" : "";
  statusItem.text = status.indexing ? "$(sync~spin) Papyrus" : `${icon} Papyrus${mode}: ${status.files}`;
  statusItem.backgroundColor =
    health === "error"
      ? new vscode.ThemeColor("statusBarItem.errorBackground")
      : health === "warning"
        ? new vscode.ThemeColor("statusBarItem.warningBackground")
        : undefined;
  // Only error and warning backgrounds are allowed for status bar items, so
  // the other states color the text.
  statusItem.color =
    health === "ok"
      ? new vscode.ThemeColor("testing.iconPassed")
      : health === "limited"
        ? new vscode.ThemeColor("charts.blue")
        : undefined;
  const problems = (status.problems ?? []).map((p) => `\n• ${p}`).join("");
  statusItem.tooltip = status.indexing
    ? "Indexing Papyrus scripts..."
    : `${status.files} scripts indexed, ${status.openFiles} open` +
      (hasManifest ? "" : "\nLimited mode: no papyrus.toml, compiling is disabled. Click to create one.") +
      problems;
  statusItem.show();
  if (status.indexing) {
    setTimeout(() => void refreshStatus(), 1000);
  }
}

async function showStatus(): Promise<void> {
  if (!client?.isRunning()) {
    void vscode.window.showWarningMessage("The Papyrus language server is not running.");
    return;
  }
  const status = await client.sendRequest(StatusRequest);
  updateStatus(status);
  const lines = [
    ...(status.problems ?? []).map((p) => `⚠ ${p}`),
    `Indexed scripts: ${status.files}${status.indexing ? " (indexing...)" : ""}`,
    `Open documents: ${status.openFiles}`,
    `Restarts: ${status.generation - 1}`,
    `Workspace folders: ${status.roots.join(", ") || "-"}`,
    `Import directories: ${status.importDirectories.join(", ") || "-"}`,
    `Flags file: ${status.flagsFile ?? "built-in Skyrim flags"}`,
    `Project: ${status.manifest ?? "no papyrus.toml (compiling is disabled)"}${
      status.manifestError ? ` (invalid: ${status.manifestError})` : ""
    }`,
    `Variables: ${
      Object.entries(status.variables ?? {})
        .map(([k, v]) => `${k} = ${v}`)
        .join(", ") || "- (set `papyrus.var` in your user settings)"
    }`,
  ];
  const restart = "Restart Server";
  const create = "Create papyrus.toml";
  const actions = status.manifest ? [restart] : [create, restart];
  const picked = await vscode.window.showInformationMessage(lines.join("\n"), { modal: true }, ...actions);
  if (picked === restart) {
    await restartServer();
  } else if (picked === create) {
    await createManifest();
  }
}

/** Writes a commented `papyrus.toml` into the first workspace folder. */
async function createManifest(): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder || !client?.isRunning()) {
    void vscode.window.showErrorMessage("Open a folder to create a papyrus.toml.");
    return;
  }
  const target = vscode.Uri.joinPath(folder.uri, "papyrus.toml");
  if (!fs.existsSync(target.fsPath)) {
    const template = await client.sendRequest(ManifestTemplateRequest);
    await vscode.workspace.fs.writeFile(target, Buffer.from(template, "utf8"));
    ignoreLocalManifest(folder.uri.fsPath);
  }
  await vscode.window.showTextDocument(target);
  // The server picks the manifest up on save; reload now for the new file.
  updateStatus(await client.sendRequest(RestartRequest, { settings: settings() }));
}

/** Adds `papyrus.local.toml` to the `.gitignore` of a git repository. */
function ignoreLocalManifest(folder: string): void {
  if (!fs.existsSync(path.join(folder, ".git"))) {
    return;
  }
  const gitignore = path.join(folder, ".gitignore");
  const current = fs.existsSync(gitignore) ? fs.readFileSync(gitignore, "utf8") : "";
  if (current.split(/\r?\n/).some((l) => l.trim() === "papyrus.local.toml")) {
    return;
  }
  const sep = current === "" || current.endsWith("\n") ? "" : "\n";
  fs.appendFileSync(gitignore, `${sep}# Machine specific papyrus.toml values\npapyrus.local.toml\n`);
}

let sources: string[] = [];

/**
 * `papyrus.compile`: from a code lens (a URI string), the explorer context
 * menu (the clicked URI and the selection) or the active editor.
 */
async function compileCommand(arg?: string | vscode.Uri, selection?: vscode.Uri[]): Promise<void> {
  let uris: vscode.Uri[];
  if (selection && selection.length > 0) {
    uris = selection;
  } else if (arg) {
    uris = [typeof arg === "string" ? vscode.Uri.parse(arg) : arg];
  } else if (vscode.window.activeTextEditor?.document.languageId === "papyrus") {
    uris = [vscode.window.activeTextEditor.document.uri];
  } else {
    void vscode.window.showWarningMessage("Open a Papyrus script to compile it.");
    return;
  }
  uris = uris.filter((u) => u.fsPath.toLowerCase().endsWith(".psc"));
  await runCompile({ type: "papyrus", files: uris.map((u) => u.fsPath) });
}

/** Picks scripts of `[build] sources` (remembering the last pick) and compiles them. */
async function compileSelected(): Promise<void> {
  const found = await Promise.all(
    sources.map((dir) =>
      vscode.workspace.findFiles(new vscode.RelativePattern(vscode.Uri.file(dir), "**/*.psc")),
    ),
  );
  const files = [...new Set(found.flat().map((u) => u.fsPath))].sort();
  if (files.length === 0) {
    void vscode.window.showWarningMessage("No scripts found in `[build] sources` of papyrus.toml.");
    return;
  }
  const previous = new Set(extensionContext.workspaceState.get<string[]>(SELECTION_KEY, []));
  const items = files.map((file) => ({
    label: path.basename(file, path.extname(file)),
    description: vscode.workspace.asRelativePath(path.dirname(file)),
    picked: previous.has(file),
    file,
  }));
  const picked = await vscode.window.showQuickPick(items, {
    canPickMany: true,
    title: "Compile Papyrus scripts",
    placeHolder: "Select the scripts to compile (the selection is remembered)",
  });
  if (!picked || picked.length === 0) {
    return;
  }
  const selected = picked.map((p) => p.file);
  await extensionContext.workspaceState.update(SELECTION_KEY, selected);
  await runCompile({ type: "papyrus", files: selected });
}

async function runCompile(definition: PapyrusTaskDefinition): Promise<void> {
  // Compile what is on screen, not the last saved version.
  await vscode.workspace.saveAll(false);
  await vscode.tasks.executeTask(compileTask(definition));
}
/** A task running the compiler commands of `papyrus.toml` in a pseudo terminal. */
function compileTask(definition: PapyrusTaskDefinition): vscode.Task {
  const files = definition.files ?? [];
  const name = definition.all
    ? "compile all"
    : files.length === 1
      ? `compile ${path.basename(files[0])}`
      : `compile ${files.length} scripts`;
  const execution = new vscode.CustomExecution(async () => new CompileTerminal(definition));
  const task = new vscode.Task(definition, vscode.TaskScope.Workspace, name, "papyrus", execution, [
    "$papyrus",
  ]);
  task.group = vscode.TaskGroup.Build;
  task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
  return task;
}

class CompileTerminal implements vscode.Pseudoterminal {
  private readonly writeEmitter = new vscode.EventEmitter<string>();
  private readonly closeEmitter = new vscode.EventEmitter<number>();
  private child: cp.ChildProcess | undefined;
  private cancelled = false;
  readonly onDidWrite = this.writeEmitter.event;
  readonly onDidClose = this.closeEmitter.event;

  constructor(private readonly definition: PapyrusTaskDefinition) {}

  open(): void {
    void this.run().then(
      (code) => this.closeEmitter.fire(code),
      (error) => {
        this.writeLine(`\x1b[31m${String(error instanceof Error ? error.message : error)}\x1b[0m`);
        this.closeEmitter.fire(1);
      },
    );
  }

  close(): void {
    this.cancelled = true;
    this.child?.kill();
  }

  private writeLine(line: string): void {
    this.writeEmitter.fire(line.replace(/\r?\n/g, "\r\n") + "\r\n");
  }

  private async run(): Promise<number> {
    if (!client?.isRunning()) {
      throw new Error("The Papyrus language server is not running.");
    }
    const params = {
      all: !!this.definition.all,
      files: (this.definition.files ?? []).map((f) => ({ uri: vscode.Uri.file(f).toString() })),
    };
    const plan = await client.sendRequest(CompileCommandsRequest, params);
    fs.mkdirSync(plan.output, { recursive: true });
    const results: { script: string; code: number }[] = [];
    for (const command of plan.commands) {
      if (this.cancelled) {
        return 130;
      }
      // The compiler does not create nested output directories itself.
      fs.mkdirSync(command.output, { recursive: true });
      this.writeLine(`\x1b[2m> ${command.commandLine}\x1b[0m`);
      // The compiler exits with -1, which Node reports as an unsigned value.
      const code = await this.spawn(command);
      results.push({ script: command.args[0], code: code > 0x7fffffff ? code - 0x100000000 : code });
    }

    // One line per script: a failure in the middle of a long output is easy to miss.
    this.writeLine("");
    for (const { script, code } of results) {
      const name = vscode.workspace.asRelativePath(script);
      this.writeLine(code === 0 ? `\x1b[32m✓\x1b[0m ${name}` : `\x1b[31m✗ ${name} (exit ${code})\x1b[0m`);
    }
    const failures = results.filter((r) => r.code !== 0).length;
    this.writeLine(
      failures === 0
        ? `\x1b[32m${results.length} compiled into ${plan.output}\x1b[0m`
        : `\x1b[31m${failures} of ${results.length} scripts failed\x1b[0m`,
    );
    if (failures > 0) {
      void vscode.window.showErrorMessage(`Papyrus: ${failures} of ${results.length} scripts failed to compile.`);
    }
    return failures === 0 ? 0 : 1;
  }

  private spawn(command: CompileCommands["commands"][number]): Promise<number> {
    return new Promise((resolve) => {
      if (!fs.existsSync(command.program)) {
        this.writeLine(`\x1b[31mcompiler not found: ${command.program} (set [compiler] path)\x1b[0m`);
        resolve(1);
        return;
      }
      const child = cp.spawn(command.program, command.args, { cwd: command.cwd, windowsHide: true });
      this.child = child;
      const forward = (data: Buffer) => this.writeEmitter.fire(data.toString().replace(/\r?\n/g, "\r\n"));
      child.stdout?.on("data", forward);
      child.stderr?.on("data", forward);
      child.on("error", (error) => {
        this.writeLine(`\x1b[31m${error.message}\x1b[0m`);
        resolve(1);
      });
      child.on("close", (code) => resolve(code ?? 1));
    });
  }
}

async function showSyntaxTree(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!client?.isRunning() || !editor || editor.document.languageId !== "papyrus") {
    return;
  }
  const tree = await client.sendRequest(SyntaxTreeRequest, {
    textDocument: { uri: editor.document.uri.toString() },
  });
  const doc = await vscode.workspace.openTextDocument({ content: tree, language: "plaintext" });
  await vscode.window.showTextDocument(doc, { viewColumn: vscode.ViewColumn.Beside, preview: true });
}
