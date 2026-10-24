import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions, State } from "vscode-languageclient/node";

import { RestartRequest } from "./protocol";
import { serverArgs, serverPath, settings } from "./settings";
import { log, state } from "./state";
import { refreshStatus, showStopped, updateStatus } from "./statusBar";

/** Starts the language server and connects the client to it. */
export async function startServer(context: vscode.ExtensionContext): Promise<void> {
  const command = serverPath(context);
  if (!fs.existsSync(command)) {
    void vscode.window.showErrorMessage(
      `Papyrus language server not found: ${command}. Build it with \`cargo build --release\` or set \`papyrus.server.path\`.`,
    );
    return;
  }

  const args = serverArgs();
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
    outputChannel: state.output,
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/{*.psc,*.flg,papyrus.toml,papyrus.local.toml}"),
    },
  };

  const client = new LanguageClient("papyrus", "Papyrus Analyzer", serverOptions, clientOptions);
  state.client = client;
  context.subscriptions.push(
    client.onDidChangeState((e) => {
      if (e.newState === State.Running) {
        void refreshStatus();
      } else if (e.newState === State.Stopped) {
        showStopped();
      }
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      // The server reloads `papyrus.toml` on save; refresh the compile button.
      if (["papyrus.toml", "papyrus.local.toml"].includes(path.basename(doc.fileName).toLowerCase())) {
        setTimeout(() => void refreshStatus(), 300);
      }
    }),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("papyrus") && client.isRunning()) {
        void client.sendNotification("workspace/didChangeConfiguration", { settings: settings() });
        setTimeout(() => void refreshStatus(), 500);
      }
    }),
  );

  try {
    await client.start();
  } catch (error) {
    log(`Failed to start the Papyrus language server: ${String(error)}`);
    void vscode.window.showErrorMessage("Failed to start the Papyrus language server. See the output for details.");
  }
}

/** Stops the server, when the extension is deactivated. */
export async function stopServer(): Promise<void> {
  await state.client?.stop();
  state.client = undefined;
}

/**
 * `papyrus.restartServer`: soft restart with `papyrus/restart`, or a new
 * process when the old one died.
 */
export async function restartServer(): Promise<void> {
  const client = state.client;
  if (!client) {
    void vscode.window.showErrorMessage("The Papyrus language server is not running.");
    return;
  }
  if (!client.isRunning()) {
    await client.start();
    return;
  }
  try {
    updateStatus(await client.sendRequest(RestartRequest, { settings: settings() }));
    void vscode.window.setStatusBarMessage("Papyrus language server restarted", 3000);
  } catch (error) {
    log(`papyrus/restart failed: ${String(error)}`);
    void vscode.window.showErrorMessage("Failed to restart the Papyrus language server.");
  }
}
