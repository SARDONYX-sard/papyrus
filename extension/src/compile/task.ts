import * as cp from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";

import { type CompileCommand, CompileCommandsRequest } from "../protocol";
import { runningClient } from "../state";

/** Task definition of `papyrus` tasks, see `taskDefinitions` in package.json. */
export interface PapyrusTaskDefinition extends vscode.TaskDefinition {
  all?: boolean;
  files?: string[];
}

/** Saves the open documents and runs a compile task. */
export async function runCompile(definition: PapyrusTaskDefinition): Promise<void> {
  // Compile what is on screen, not the last saved version.
  await vscode.workspace.saveAll(false);
  await vscode.tasks.executeTask(compileTask(definition));
}

/** A task running the compiler commands of `papyrus.toml` in a pseudo terminal. */
export function compileTask(definition: PapyrusTaskDefinition): vscode.Task {
  const files = definition.files ?? [];
  const name = definition.all
    ? "compile all"
    : files.length === 1
      ? `compile ${path.basename(files[0])}`
      : `compile ${files.length} scripts`;
  const execution = new vscode.CustomExecution(async () => new CompileTerminal(definition));
  const task = new vscode.Task(definition, vscode.TaskScope.Workspace, name, "papyrus", execution, ["$papyrus"]);
  task.group = vscode.TaskGroup.Build;
  task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
  return task;
}

/**
 * Runs the compiler once per script (the compiler cannot compile several
 * files at once), then prints one result line per script.
 */
class CompileTerminal implements vscode.Pseudoterminal {
  private readonly writeEmitter = new vscode.EventEmitter<string>();
  private readonly closeEmitter = new vscode.EventEmitter<number>();
  private child: cp.ChildProcess | undefined;
  private cancelled = false;
  readonly onDidWrite = this.writeEmitter.event;
  readonly onDidClose = this.closeEmitter.event;

  constructor(private readonly definition: PapyrusTaskDefinition) {}

  open(): void {
    void this.run().then(
      (code) => this.closeEmitter.fire(code),
      (error: unknown) => {
        this.writeLine(`\x1b[31m${error instanceof Error ? error.message : String(error)}\x1b[0m`);
        this.closeEmitter.fire(1);
      },
    );
  }

  close(): void {
    this.cancelled = true;
    this.child?.kill();
  }

  private writeLine(line: string): void {
    this.writeEmitter.fire(line.replace(/\r?\n/g, "\r\n") + "\r\n");
  }

  private async run(): Promise<number> {
    const client = runningClient();
    if (!client) {
      throw new Error("The Papyrus language server is not running.");
    }
    const params = {
      all: !!this.definition.all,
      files: (this.definition.files ?? []).map((f) => ({ uri: vscode.Uri.file(f).toString() })),
    };
    const plan = await client.sendRequest(CompileCommandsRequest, params);
    fs.mkdirSync(plan.output, { recursive: true });

    const results: { script: string; code: number }[] = [];
    for (const command of plan.commands) {
      if (this.cancelled) {
        return 130;
      }
      // The compiler does not create nested output directories itself.
      fs.mkdirSync(command.output, { recursive: true });
      this.writeLine(`\x1b[2m> ${command.commandLine}\x1b[0m`);
      // The compiler exits with -1, which Node reports as an unsigned value.
      const code = await this.spawn(command);
      results.push({ script: command.args[0], code: code > 0x7fffffff ? code - 0x100000000 : code });
    }

    // One line per script: a failure in the middle of a long output is easy to miss.
    this.writeLine("");
    for (const { script, code } of results) {
      const name = vscode.workspace.asRelativePath(script);
      this.writeLine(code === 0 ? `\x1b[32m✓\x1b[0m ${name}` : `\x1b[31m✗ ${name} (exit ${code})\x1b[0m`);
    }
    const failures = results.filter((r) => r.code !== 0).length;
    this.writeLine(
      failures === 0
        ? `\x1b[32m${results.length} compiled into ${plan.output}\x1b[0m`
        : `\x1b[31m${failures} of ${results.length} scripts failed\x1b[0m`,
    );
    if (failures > 0) {
      void vscode.window.showErrorMessage(`Papyrus: ${failures} of ${results.length} scripts failed to compile.`);
    }
    return failures === 0 ? 0 : 1;
  }

  private spawn(command: CompileCommand): Promise<number> {
    return new Promise((resolve) => {
      if (!fs.existsSync(command.program)) {
        this.writeLine(`\x1b[31mcompiler not found: ${command.program} (set [compiler] path)\x1b[0m`);
        resolve(1);
        return;
      }
      const child = cp.spawn(command.program, command.args, { cwd: command.cwd, windowsHide: true });
      this.child = child;
      const forward = (data: Buffer) => this.writeEmitter.fire(data.toString().replace(/\r?\n/g, "\r\n"));
      child.stdout?.on("data", forward);
      child.stderr?.on("data", forward);
      child.on("error", (error) => {
        this.writeLine(`\x1b[31m${error.message}\x1b[0m`);
        resolve(1);
      });
      child.on("close", (code) => resolve(code ?? 1));
    });
  }
}
