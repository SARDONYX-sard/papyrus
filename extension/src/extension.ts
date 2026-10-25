import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";

import { compileAll, compileCommand, compileSelected } from "./compile/commands";
import { compileTask, type PapyrusTaskDefinition } from "./compile/task";
import { registerDecompiler } from "./decompile";
import { createManifest } from "./manifest";
import { registerScriptsView } from "./scriptsView";
import { restartServer, startServer, stopServer } from "./server";
import { serverPath } from "./settings";
import { state } from "./state";
import { createStatusBar } from "./statusBar";
import { showStatus } from "./statusCommand";
import { showSyntaxTree } from "./syntaxTree";
import { showReferences } from "./references";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  state.context = context;
  state.output = vscode.window.createOutputChannel("Papyrus Analyzer", { log: true });
  context.subscriptions.push(state.output);
  createStatusBar(context);
  registerDecompiler(context);
  registerScriptsView(context);

  context.subscriptions.push(
    vscode.commands.registerCommand("papyrus.restartServer", restartServer),
    vscode.commands.registerCommand("papyrus.showStatus", showStatus),
    vscode.commands.registerCommand("papyrus.showReferences", showReferences),
    vscode.commands.registerCommand("papyrus.showSyntaxTree", showSyntaxTree),
    vscode.commands.registerCommand("papyrus.compile", compileCommand),
    vscode.commands.registerCommand("papyrus.compileAll", compileAll),
    vscode.commands.registerCommand("papyrus.compileSelected", compileSelected),
    vscode.commands.registerCommand("papyrus.createManifest", createManifest),
    // The `▶ Run test` code lens: `papyrus_analyzer run Script.Function` in a
    // terminal, from the folder of the papyrus.toml of the script.
    vscode.commands.registerCommand("papyrus.runTest", async (uri: string, target: string) => {
      const file = vscode.Uri.parse(uri).fsPath;
      let dir = path.dirname(file);
      while (!fs.existsSync(path.join(dir, "papyrus.toml")) && path.dirname(dir) !== dir) {
        dir = path.dirname(dir);
      }
      if (!fs.existsSync(path.join(dir, "papyrus.toml"))) {
        dir = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file))?.uri.fsPath ?? path.dirname(file);
      }
      const execution = new vscode.ProcessExecution(serverPath(context), ["run", target], { cwd: dir });
      const task = new vscode.Task({ type: "papyrus-run" }, vscode.TaskScope.Workspace, `run ${target}`, "papyrus", execution);
      task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
      await vscode.tasks.executeTask(task);
    }),
    vscode.tasks.registerTaskProvider("papyrus", {
      provideTasks: () => [compileTask({ type: "papyrus", all: true })],
      resolveTask: (task) => compileTask(task.definition as PapyrusTaskDefinition),
    }),
  );

  await startServer(context);
}

export async function deactivate(): Promise<void> {
  await stopServer();
}
