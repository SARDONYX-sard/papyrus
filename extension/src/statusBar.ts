import * as vscode from "vscode";

import { type ServerStatus, StatusRequest } from "./protocol";
import { runningClient, state } from "./state";

let statusItem: vscode.StatusBarItem;
/** `$(run-all) Compile…`: compiles the scripts picked with a quick pick. */
let compileItem: vscode.StatusBarItem;

/** Creates the status bar items. */
export function createStatusBar(context: vscode.ExtensionContext): void {
  statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  statusItem.command = "papyrus.showStatus";
  compileItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, -1);
  compileItem.command = "papyrus.compileSelected";
  compileItem.text = "$(run-all) Compile…";
  compileItem.tooltip = "Papyrus: pick the scripts to compile together";
  context.subscriptions.push(statusItem, compileItem);
}

/** Asks the server for its status and shows it. */
export async function refreshStatus(): Promise<void> {
  const client = runningClient();
  if (!client) {
    return;
  }
  try {
    updateStatus(await client.sendRequest(StatusRequest));
  } catch {
    // Older servers do not implement `papyrus/status`.
  }
}

/** Shows that the server process stopped. */
export function showStopped(): void {
  statusItem.text = "$(error) Papyrus";
  statusItem.backgroundColor = new vscode.ThemeColor("statusBarItem.errorBackground");
  statusItem.color = undefined;
  statusItem.tooltip = "The Papyrus language server stopped. Run `Papyrus: Restart Language Server`.";
  statusItem.show();
}

/** Shows a status of the server. */
export function updateStatus(status: ServerStatus): void {
  state.hasManifest = !!status.manifest;
  state.sources = status.sources ?? [];
  void vscode.commands.executeCommand("setContext", "papyrus.hasManifest", state.hasManifest);
  if (state.hasManifest) {
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
      (state.hasManifest ? "" : "\nLimited mode: no papyrus.toml, compiling is disabled. Click to create one.") +
      problems;
  statusItem.show();

  if (status.indexing) {
    setTimeout(() => void refreshStatus(), 1000);
  }
}
