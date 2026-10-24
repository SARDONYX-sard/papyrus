import * as vscode from "vscode";

import { SyntaxTreeRequest } from "./protocol";
import { runningClient } from "./state";

/** `papyrus.showSyntaxTree`: opens the syntax tree of the active script beside it. */
export async function showSyntaxTree(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  const client = runningClient();
  if (!client || !editor || editor.document.languageId !== "papyrus") {
    return;
  }
  const tree = await client.sendRequest(SyntaxTreeRequest, {
    textDocument: { uri: editor.document.uri.toString() },
  });
  const doc = await vscode.workspace.openTextDocument({ content: tree, language: "plaintext" });
  await vscode.window.showTextDocument(doc, { viewColumn: vscode.ViewColumn.Beside, preview: true });
}
