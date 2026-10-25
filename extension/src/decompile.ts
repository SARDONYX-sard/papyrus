import * as path from "node:path";
import * as vscode from "vscode";

import { DecompileRequest } from "./protocol";
import { runningClient } from "./state";
import type { LanguageClient } from "vscode-languageclient/node";

/**
 * Read-only documents of decompiled `.pex` files:
 * `papyrus-pex:/dir/Name (decompiled).psc?<uri of the .pex>` for the source,
 * `papyrus-pex:/dir/Name (assembly).pas?<uri of the .pex>` for the assembly.
 * The `.psc` extension gives the source the Papyrus language. So that they
 * are not mistaken for the original scripts, the tab says `(decompiled)`
 * with a badge, and the source starts with a comment saying so.
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
  readonly changed = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.changed.event;

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const c = await client();
    if (!c) {
      return "; the Papyrus language server is not running";
    }
    try {
      return await c.sendRequest(DecompileRequest, { uri: uri.query, assembly: uri.path.endsWith(".pas") });
    } catch (e) {
      return `; ${vscode.Uri.parse(uri.query).fsPath}: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
}

/** The virtual document of a `.pex` file. */
function pexDocumentUri(pex: vscode.Uri, assembly: boolean): vscode.Uri {
  const dir = path.posix.dirname(pex.path);
  const name = path.posix.basename(pex.path, path.posix.extname(pex.path));
  return vscode.Uri.from({
    scheme: PEX_SCHEME,
    path: assembly ? `${dir}/${name} (assembly).pas` : `${dir}/${name} (decompiled).psc`,
    query: pex.toString(),
  });
}

/**
 * The file of a command: the clicked one (a URI, or a row of the scripts
 * view), else the active decompiled view or script.
 */
function targetPex(arg?: vscode.Uri | { uri: vscode.Uri }): vscode.Uri | undefined {
  if (arg) {
    return arg instanceof vscode.Uri ? arg : arg.uri;
  }
  const active = vscode.window.activeTextEditor?.document;
  if (active?.uri.scheme === PEX_SCHEME) {
    return vscode.Uri.parse(active.uri.query);
  }
  if (active?.uri.scheme === "file" && active.languageId === "papyrus") {
    return active.uri;
  }
  return undefined;
}

async function open(
  arg: vscode.Uri | { uri: vscode.Uri } | undefined,
  assembly: boolean,
  column?: vscode.ViewColumn,
): Promise<void> {
  const pex = targetPex(arg);
  if (!pex) {
    void vscode.window.showWarningMessage(
      assembly ? "Select a .pex or .psc file to show its assembly." : "Select a .pex file to decompile.",
    );
    return;
  }
  // The source of a script is the script itself.
  if (pex.path.toLowerCase().endsWith(".psc") && !assembly) {
    await vscode.window.showTextDocument(pex, { viewColumn: column });
    return;
  }
  let doc = await vscode.workspace.openTextDocument(pexDocumentUri(pex, assembly));
  // `.pas` is Pascal too: an extension for it may win over the file pattern.
  if (assembly && doc.languageId !== "papyrus-assembly") {
    doc = await vscode.languages.setTextDocumentLanguage(doc, "papyrus-assembly");
  }
  await vscode.window.showTextDocument(doc, { preview: true, viewColumn: column });
}

/** `papyrus.decompile`: shows the source of a `.pex` file. */
export async function decompileCommand(arg?: vscode.Uri | { uri: vscode.Uri }): Promise<void> {
  await open(arg, false);
}

/**
 * `papyrus.showAssembly`: shows the assembly of a `.pex` file, or of a
 * `.psc` file compiled in memory.
 */
export async function showAssemblyCommand(arg?: vscode.Uri | { uri: vscode.Uri }): Promise<void> {
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

/** A badge on the tabs of decompiled documents. */
class PexDecorationProvider implements vscode.FileDecorationProvider {
  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== PEX_SCHEME) {
      return undefined;
    }
    const from = vscode.Uri.parse(uri.query).fsPath;
    if (uri.path.endsWith(".pas")) {
      const how = from.toLowerCase().endsWith(".psc") ? "Compiled in memory from" : "Disassembled from";
      return { badge: "A", tooltip: `${how} ${from}, read-only`, color: new vscode.ThemeColor("charts.purple") };
    }
    return {
      badge: "D",
      tooltip: `Decompiled from ${from}, read-only: not the original source`,
      color: new vscode.ThemeColor("charts.purple"),
    };
  }
}

/** Registers the provider, the editor and the commands. */
export function registerDecompiler(context: vscode.ExtensionContext): void {
  const content = new PexContentProvider();
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(PEX_SCHEME, content),
    // The assembly of a script follows its edits, on save.
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "papyrus" && doc.uri.scheme === "file") {
        content.changed.fire(pexDocumentUri(doc.uri, true));
      }
    }),
    vscode.window.registerFileDecorationProvider(new PexDecorationProvider()),
    vscode.window.registerCustomEditorProvider(PEX_VIEW_TYPE, new PexEditorProvider()),
    vscode.commands.registerCommand("papyrus.decompile", decompileCommand),
    vscode.commands.registerCommand("papyrus.showAssembly", showAssemblyCommand),
  );
}
