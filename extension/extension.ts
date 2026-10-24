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

let client: LanguageClient | undefined;
let statusItem: vscode.StatusBarItem;
let output: vscode.OutputChannel;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  output = vscode.window.createOutputChannel("Papyrus Language Server");
  statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  statusItem.command = "papyrus.showStatus";
  context.subscriptions.push(output, statusItem);

  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restartServer", restartServer),
    vscode.commands.registerCommand("papyrus.showStatus", showStatus),
    vscode.commands.registerCommand("papyrus.showSyntaxTree", showSyntaxTree),
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
  const exe = process.platform === "win32" ? "papyrus_lsp.exe" : "papyrus_lsp";
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

  const serverOptions: ServerOptions = {
    run: { command },
    debug: { command, options: { env: { ...process.env, RUST_LOG: "debug" } } },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "papyrus" }],
    initializationOptions: settings(),
    outputChannel: output,
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.psc"),
    },
  };

  client = new LanguageClient("papyrus", "Papyrus Language Server", serverOptions, clientOptions);
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
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("papyrus") && client?.isRunning()) {
        void client.sendNotification("workspace/didChangeConfiguration", { settings: settings() });
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

function updateStatus(status: ServerStatus): void {
  statusItem.text = status.indexing ? "$(sync~spin) Papyrus" : `$(check) Papyrus: ${status.files}`;
  statusItem.tooltip = status.indexing
    ? "Indexing Papyrus scripts..."
    : `${status.files} scripts indexed, ${status.openFiles} open`;
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
    `Indexed scripts: ${status.files}${status.indexing ? " (indexing...)" : ""}`,
    `Open documents: ${status.openFiles}`,
    `Restarts: ${status.generation - 1}`,
    `Workspace folders: ${status.roots.join(", ") || "-"}`,
    `Import directories: ${status.importDirectories.join(", ") || "-"}`,
  ];
  const restart = "Restart Server";
  const picked = await vscode.window.showInformationMessage(lines.join("\n"), { modal: true }, restart);
  if (picked === restart) {
    await restartServer();
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
