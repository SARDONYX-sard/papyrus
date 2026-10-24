import * as vscode from "vscode";

import { createManifest } from "./manifest";
import { StatusRequest } from "./protocol";
import { restartServer } from "./server";
import { runningClient } from "./state";
import { updateStatus } from "./statusBar";

/** `papyrus.showStatus`: everything the server knows, with follow-up actions. */
export async function showStatus(): Promise<void> {
  const client = runningClient();
  if (!client) {
    void vscode.window.showWarningMessage("The Papyrus language server is not running.");
    return;
  }
  const status = await client.sendRequest(StatusRequest);
  updateStatus(status);
  const variables = Object.entries(status.variables ?? {})
    .map(([k, v]) => `${k} = ${v}`)
    .join(", ");
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
    `Variables: ${variables || "- (set `papyrus.var` in your user settings)"}`,
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
