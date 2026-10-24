import * as path from "node:path";
import * as vscode from "vscode";

import { DecompileRequest } from "./protocol";
import { runningClient } from "./state";
import type { LanguageClient } from "vscode-languageclient/node";

/**
 * Read-only documents of decompiled `.pex` files:
 * `papyrus-pex:/dir/Name.psc?<uri of the .pex>` for the source,
 * `papyrus-pex:/dir/Name.pas?<uri of the .pex>` for the assembly. The `.psc`
 * extension gives the source the Papyrus language.
 */
export const PEX_SCHEME = "papyrus-pex";

/** The view type of the `.pex` editor, see `contributes.customEditors`. */
export const PEX_VIEW_TYPE = "papyrus.pexViewer";

/** Waits for the server, which may still be starting when a `.pex` opens. */
export async function client(): Promise<LanguageClient | undefined> {
  for (let i = 0; i < 100; i++) {
    const c = runningClient();
    if (c) {
      return c;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return undefined;
}

class PexContentProvider implements vscode.TextDocumentContentProvider {
  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const c = await client();
    if (!c) {
      return "; the Papyrus language server is not running";
    }
    try {
      return await c.sendRequest(DecompileRequest, { uri: uri.query, assembly: uri.path.endsWith(".pas") });
    } catch (e) {
      return `; cannot decompile ${vscode.Uri.parse(uri.query).fsPath}: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
}

/** The virtual document of a `.pex` file. */
function pexDocumentUri(pex: vscode.Uri, assembly: boolean): vscode.Uri {
  const dir = path.posix.dirname(pex.path);
  const name = path.posix.basename(pex.path, path.posix.extname(pex.path));
  return vscode.Uri.from({
    scheme: PEX_SCHEME,
    path: `${dir}/${name}.${assembly ? "pas" : "psc"}`,
    query: pex.toString(),
  });
}

/** The `.pex` file of a command: the clicked one, or the active decompiled view. */
function targetPex(arg?: vscode.Uri): vscode.Uri | undefined {
  if (arg) {
    return arg;
  }
  const active = vscode.window.activeTextEditor?.document.uri;
  if (active?.scheme === PEX_SCHEME) {
    return vscode.Uri.parse(active.query);
  }
  return undefined;
}

async function open(arg: vscode.Uri | undefined, assembly: boolean, column?: vscode.ViewColumn): Promise<void> {
  const pex = targetPex(arg);
  if (!pex) {
    void vscode.window.showWarningMessage("Select a .pex file to decompile.");
    return;
  }
  const doc = await vscode.workspace.openTextDocument(pexDocumentUri(pex, assembly));
  await vscode.window.showTextDocument(doc, { preview: true, viewColumn: column });
}

/** `papyrus.decompile`: shows the source of a `.pex` file. */
export async function decompileCommand(arg?: vscode.Uri): Promise<void> {
  await open(arg, false);
}

/** `papyrus.showAssembly`: shows the assembly of a `.pex` file. */
export async function showAssemblyCommand(arg?: vscode.Uri): Promise<void> {
  await open(arg, true, arg ? undefined : vscode.ViewColumn.Beside);
}

/**
 * Opening a `.pex` file shows its source: the custom editor hands over to the
 * decompiled document and closes itself.
 */
class PexEditorProvider implements vscode.CustomReadonlyEditorProvider {
  openCustomDocument(uri: vscode.Uri): vscode.CustomDocument {
    return { uri, dispose: () => {} };
  }

  async resolveCustomEditor(document: vscode.CustomDocument, panel: vscode.WebviewPanel): Promise<void> {
    const column = panel.viewColumn;
    await open(document.uri, false, column);
    panel.dispose();
  }
}

/** Registers the provider, the editor and the commands. */
export function registerDecompiler(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(PEX_SCHEME, new PexContentProvider()),
    vscode.window.registerCustomEditorProvider(PEX_VIEW_TYPE, new PexEditorProvider()),
    vscode.commands.registerCommand("papyrus.decompile", decompileCommand),
    vscode.commands.registerCommand("papyrus.showAssembly", showAssemblyCommand),
  );
}
