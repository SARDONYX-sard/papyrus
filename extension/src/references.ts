import * as vscode from "vscode";
import type * as lsp from "vscode-languageclient/node";

import { runningClient } from "./state";

/**
 * `papyrus.showReferences`: the command of the `overridden in N scripts` code
 * lenses. The server sends protocol values; `editor.action.showReferences`
 * takes editor ones.
 */
export async function showReferences(uri: string, position: lsp.Position, locations: lsp.Location[]): Promise<void> {
  const client = runningClient();
  if (!client) {
    return;
  }
  const c = client.protocol2CodeConverter;
  await vscode.commands.executeCommand(
    "editor.action.showReferences",
    c.asUri(uri),
    c.asPosition(position),
    locations.map((l) => c.asLocation(l)),
  );
}
