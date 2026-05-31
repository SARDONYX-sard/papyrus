import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import { LanguageClient, LanguageClientOptions, ServerOptions } from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
  const serverPath = context.asAbsolutePath(path.join("dist", "papyrus_lsp.exe"));
  if (!fs.existsSync(serverPath)) {
    vscode.window.showErrorMessage(`LSP not found: ${serverPath}`);
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath },
    debug: { command: serverPath },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "papyrus" }],
  };

  client = new LanguageClient("papyrus-lsp", "Papyrus LSP", serverOptions, clientOptions);

  client.start();

  context.subscriptions.push(client);
}

export function deactivate() {
  if (client) client.stop();
}
