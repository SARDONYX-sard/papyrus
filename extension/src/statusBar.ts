import * as vscode from "vscode";

import { type ServerStatus, StatusRequest } from "./protocol";
import { runningClient, type ServerSummary, scriptScope, state } from "./state";

let statusItem: vscode.StatusBarItem;
/** `$(run-all) Compile…`: compiles the scripts picked with a quick pick. */
let compileItem: vscode.StatusBarItem;
/** `$(info) Limited`: the active script is not part of a project. */
let fileItem: vscode.StatusBarItem;

/** Creates the status bar items. */
export function createStatusBar(context: vscode.ExtensionContext): void {
  statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  statusItem.command = "papyrus.showStatus";
  compileItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, -1);
  compileItem.command = "papyrus.compileSelected";
  compileItem.text = "$(run-all) Compile…";
  compileItem.tooltip = "Papyrus: pick the scripts to compile together";
  fileItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, -2);
  fileItem.text = "$(info) Limited";
  fileItem.color = new vscode.ThemeColor("charts.blue");
  context.subscriptions.push(
    statusItem,
    compileItem,
    fileItem,
    vscode.window.onDidChangeActiveTextEditor(updateFileItem),
  );
}

/**
 * Shows `Limited` while the active script is outside of the sources of
 * papyrus.toml: the server checks it only while open, and compiles it
 * without the settings of a project.
 */
function updateFileItem(): void {
  const doc = vscode.window.activeTextEditor?.document;
  if (!doc || doc.uri.scheme !== "file" || doc.languageId !== "papyrus" || scriptScope(doc.uri.fsPath) !== "limited") {
    fileItem.hide();
    return;
  }
  if (state.manifest) {
    fileItem.tooltip =
      "Limited: this script is in neither `[build] sources` nor `[test] sources` of papyrus.toml. " +
      "It is checked only while open, and compiles with the settings of the first project. Click to open papyrus.toml.";
    fileItem.command = { command: "vscode.open", title: "Open papyrus.toml", arguments: [vscode.Uri.file(state.manifest)] };
  } else {
    fileItem.tooltip =
      "Limited mode: no papyrus.toml, scripts compile with the built-in compiler and the editor settings. Click to create one.";
    fileItem.command = "papyrus.createManifest";
  }
  fileItem.show();
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
  const tooltip = "The Papyrus language server stopped. Run `Papyrus: Restart Language Server`.";
  statusItem.text = "$(error) Papyrus";
  statusItem.backgroundColor = new vscode.ThemeColor("statusBarItem.errorBackground");
  statusItem.color = undefined;
  statusItem.tooltip = tooltip;
  statusItem.show();
  setServer({ label: "Papyrus: stopped", icon: "error", color: "testing.iconFailed", tooltip });
}

/** Gives the explorer the new status. */
function setServer(summary: ServerSummary): void {
  state.server = summary;
  for (const listener of state.serverListeners) {
    listener();
  }
}

/** Shows a status of the server. */
export function updateStatus(status: ServerStatus): void {
  state.hasManifest = !!status.manifest;
  state.manifest = status.manifest ?? undefined;
  // The explorer lists other files when these change.
  const before = JSON.stringify([state.sources, state.testSources, state.buildDirs]);
  state.sources = status.sources ?? [];
  state.testSources = status.testSources ?? [];
  state.buildDirs = status.buildDirs ?? [];
  if (JSON.stringify([state.sources, state.testSources, state.buildDirs]) !== before) {
    for (const listener of state.statusListeners) {
      listener();
    }
  }
  updateFileItem();
  void vscode.commands.executeCommand("setContext", "papyrus.hasManifest", state.hasManifest);
  // Without papyrus.toml, the built-in compiler still compiles.
  compileItem.show();

  // Red for errors, yellow for warnings, blue without papyrus.toml (limited
  // mode: the built-in compiler with the editor settings), green when
  // everything resolves.
  const health = status.health ?? "ok";
  const icon = { ok: "check", limited: "info", warning: "warning", error: "error" }[health];
  const mode = health === "limited" ? " (limited)" : "";

  // Everything running, in this one item: the first in the text, all on hover.
  const activities: { short: string; long: string }[] = [];
  if (status.indexing) {
    activities.push({ short: "indexing", long: "Indexing the scripts" });
  }
  const checking = status.checking;
  if (checking) {
    const left = checking.remainingMs != null ? `, about ${Math.ceil(checking.remainingMs / 1000)}s left` : "";
    const last = checking.current ? ` (${checking.current})` : "";
    activities.push({
      short: `checking ${checking.done}/${checking.total}`,
      long: `Checking the scripts: ${checking.done} of ${checking.total}${left}${last}`,
    });
  }
  const more = activities.length > 1 ? ` +${activities.length - 1}` : "";
  const label =
    activities.length > 0 ? `Papyrus: ${activities[0].short}${more}` : `Papyrus${mode}: ${status.files} scripts`;
  statusItem.text = activities.length > 0 ? `$(sync~spin) ${label}` : `$(${icon}) Papyrus${mode}: ${status.files}`;
  statusItem.backgroundColor =
    health === "error"
      ? new vscode.ThemeColor("statusBarItem.errorBackground")
      : health === "warning"
        ? new vscode.ThemeColor("statusBarItem.warningBackground")
        : undefined;
  // Only error and warning backgrounds are allowed for status bar items, so
  // the other states color the text.
  const color = { ok: "testing.iconPassed", limited: "charts.blue", warning: undefined, error: undefined }[health];
  statusItem.color = color && new vscode.ThemeColor(color);

  const lines = activities.map((a) => `$(sync~spin) ${a.long}`);
  lines.push(`${status.files} scripts indexed, ${status.openFiles} open`);
  if (!state.hasManifest) {
    lines.push(
      "Limited mode: no papyrus.toml, scripts compile with the built-in compiler and the editor settings. Click to create one.",
    );
  }
  lines.push(...(status.problems ?? []).map((p) => `• ${p}`));
  // Markdown with icons; two spaces end a line.
  const tooltip = lines.join("  \n");
  statusItem.tooltip = new vscode.MarkdownString(tooltip, true);
  // The status bar shows the work in progress and the errors; the explorer
  // always shows the status.
  if (activities.length > 0 || health === "error") {
    statusItem.show();
  } else {
    statusItem.hide();
  }
  setServer({
    label,
    icon: activities.length > 0 ? "sync~spin" : icon,
    // The tree has no backgrounds: the icons of warnings and errors get colors.
    color:
      activities.length > 0
        ? undefined
        : (color ?? (health === "error" ? "testing.iconFailed" : "problemsWarningIcon.foreground")),
    tooltip,
  });

  if (activities.length > 0) {
    setTimeout(() => void refreshStatus(), checking ? 300 : 1000);
  }
}
