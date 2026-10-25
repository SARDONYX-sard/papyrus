import * as path from "node:path";
import * as vscode from "vscode";

/** The `papyrus` settings section, sent to the server. */
export function settings(): Record<string, unknown> {
  const config = vscode.workspace.getConfiguration("papyrus");
  return {
    importDirectories: config.get<string[]>("importDirectories", []).map(expandPath),
    semanticDiagnostics: config.get<boolean>("semanticDiagnostics", true),
    flagsFile: expandPath(config.get<string>("flagsFile", "")) || null,
    var: config.get<Record<string, string>>("var", {}),
    inlayHints: {
      parameterNames: config.get<boolean>("inlayHints.parameterNames", true),
      overrides: config.get<boolean>("inlayHints.overrides", true),
    },
  };
}

/** Expands `~` and `${workspaceFolder}` in a configured path. */
export function expandPath(p: string): string {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
  const home = process.env.HOME ?? process.env.USERPROFILE ?? "";
  return p.replace("${workspaceFolder}", folder).replace(/^~(?=$|[\\/])/, home);
}

/** The server executable: `papyrus.server.path`, or the bundled one. */
export function serverPath(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace.getConfiguration("papyrus").get<string>("server.path");
  if (configured) {
    return expandPath(configured);
  }
  const exe = process.platform === "win32" ? "papyrus_analyzer.exe" : "papyrus_analyzer";
  return context.asAbsolutePath(path.join("dist", exe));
}

/** Extra command line arguments of the server. */
export function serverArgs(): string[] {
  return vscode.workspace.getConfiguration("papyrus").get<string[]>("server.extraArgs", []);
}
