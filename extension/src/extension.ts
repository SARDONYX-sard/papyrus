import * as vscode from "vscode";

import { compileAll, compileCommand, compileSelected } from "./compile/commands";
import { compileTask, type PapyrusTaskDefinition } from "./compile/task";
import { registerDecompiler } from "./decompile";
import { createManifest } from "./manifest";
import { registerScriptsView } from "./scriptsView";
import { restartServer, startServer, stopServer } from "./server";
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
