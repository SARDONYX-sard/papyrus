import * as path from "node:path";
import * as vscode from "vscode";

import { compileEvents, runCompile } from "./compile/task";
import { client } from "./decompile";
import { DecompileFilesRequest } from "./protocol";
import { state } from "./state";

/**
 * The Papyrus scripts view: the scripts of the workspace as a tree, in one of
 * two modes, switched from the title bar:
 *
 * - compile: the `.psc` sources (`[build] sources` of papyrus.toml, else the
 *   workspace folders); a click compiles a script;
 * - decompile: the `.pex` files of the workspace folders; a click decompiles
 *   one and opens the result.
 *
 * Folders have a button running the action on everything below them. Each
 * script shows the result of its last run. Outputs keep the directory
 * structure: `[build] output` of papyrus.toml (`decompile` below it), else
 * `build/papyrus/compile` and `build/papyrus/decompile` of the workspace
 * folder.
 */

type Mode = "compile" | "decompile";
type Status = "running" | "passed" | "failed";

const MODE_KEY = "papyrus.scriptsView.mode";

interface FolderNode {
  kind: "folder";
  uri: vscode.Uri;
  name: string;
  children: (FolderNode | FileNode)[];
}

interface FileNode {
  kind: "file";
  uri: vscode.Uri;
}

/** The first row: the current mode, in capitals; a click switches it. */
interface ModeNode {
  kind: "mode";
}

type Node = ModeNode | FolderNode | FileNode;

/** Path comparison key; Windows paths ignore case. */
function key(p: string): string {
  const n = path.normalize(p);
  return process.platform === "win32" ? n.toLowerCase() : n;
}

function isBelow(file: string, dir: string): boolean {
  const rel = path.relative(key(dir), key(file));
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

class ScriptsProvider implements vscode.TreeDataProvider<Node> {
  private readonly changed = new vscode.EventEmitter<Node | undefined>();
  readonly onDidChangeTreeData = this.changed.event;
  mode: Mode;
  private roots: Node[] = [];
  private loaded = false;
  private readonly statuses = new Map<string, Status>();
  private readonly errors = new Map<string, string>();

  constructor(private readonly memento: vscode.Memento) {
    this.mode = memento.get<Mode>(MODE_KEY, "compile");
  }

  async setMode(mode: Mode): Promise<void> {
    this.mode = mode;
    await this.memento.update(MODE_KEY, mode);
    this.refresh();
  }

  refresh(): void {
    this.loaded = false;
    this.changed.fire(undefined);
  }

  setStatus(file: string, status: Status, error?: string): void {
    this.statuses.set(key(file), status);
    if (error) {
      this.errors.set(key(file), error);
    } else {
      this.errors.delete(key(file));
    }
    this.changed.fire(undefined);
  }

  /** The directories whose scripts are listed, and those to leave out. */
  private dirs(): { include: string[]; exclude: string[] } {
    const folders = (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
    const build = state.buildDirs.flatMap((b) => [b.compile, b.decompile]);
    const defaults = folders.map((f) => path.join(f, "build", "papyrus"));
    if (this.mode === "compile") {
      // Decompiled scripts are not sources.
      return { include: state.sources.length > 0 ? state.sources : folders, exclude: [...build, ...defaults] };
    }
    return { include: folders, exclude: [] };
  }

  private async load(): Promise<void> {
    const { include, exclude } = this.dirs();
    const pattern = this.mode === "compile" ? "**/*.psc" : "**/*.pex";
    const found = await Promise.all(
      include.map((dir) =>
        vscode.workspace.findFiles(new vscode.RelativePattern(vscode.Uri.file(dir), pattern), "**/node_modules/**"),
      ),
    );
    const roots: FolderNode[] = include.map((dir) => ({
      kind: "folder",
      uri: vscode.Uri.file(dir),
      name: path.basename(dir),
      children: [],
    }));
    const seen = new Set<string>();
    found.forEach((files, i) => {
      for (const file of files) {
        const k = key(file.fsPath);
        if (seen.has(k) || exclude.some((dir) => isBelow(file.fsPath, dir))) {
          continue;
        }
        seen.add(k);
        insert(roots[i], file);
      }
    });
    for (const root of roots) {
      sort(root);
    }
    const nonEmpty = roots.filter((r) => r.children.length > 0);
    // One folder: show its content directly.
    this.roots = nonEmpty.length === 1 ? nonEmpty[0].children : nonEmpty;
    this.loaded = true;
  }

  async getChildren(node?: Node): Promise<Node[]> {
    if (!node) {
      if (!this.loaded) {
        await this.load();
      }
      return [{ kind: "mode" }, ...this.roots];
    }
    return node.kind === "folder" ? node.children : [];
  }

  getTreeItem(node: Node): vscode.TreeItem {
    if (node.kind === "mode") {
      const compile = this.mode === "compile";
      const item = new vscode.TreeItem(compile ? "COMPILE MODE" : "DECOMPILE MODE");
      item.contextValue = "papyrusMode";
      item.description = compile ? ".psc → .pex  (click: switch to decompile)" : ".pex → .psc  (click: switch to compile)";
      item.iconPath = new vscode.ThemeIcon(
        compile ? "file-code" : "file-binary",
        new vscode.ThemeColor(compile ? "charts.green" : "charts.purple"),
      );
      item.tooltip = compile
        ? "Compile mode: a click on a script compiles it. Click here to switch to decompile mode."
        : "Decompile mode: a click on a .pex file decompiles it. Click here to switch to compile mode.";
      item.command = {
        command: compile ? "papyrus.scripts.decompileMode" : "papyrus.scripts.compileMode",
        title: "Switch mode",
      };
      return item;
    }
    if (node.kind === "folder") {
      const item = new vscode.TreeItem(node.name, vscode.TreeItemCollapsibleState.Expanded);
      item.resourceUri = node.uri;
      item.contextValue = "papyrusFolder";
      item.iconPath = vscode.ThemeIcon.Folder;
      return item;
    }
    const item = new vscode.TreeItem(node.uri, vscode.TreeItemCollapsibleState.None);
    item.contextValue = "papyrusFile";
    const status = this.statuses.get(key(node.uri.fsPath));
    const error = this.errors.get(key(node.uri.fsPath));
    if (status === "running") {
      item.iconPath = new vscode.ThemeIcon("loading~spin");
    } else if (status === "passed") {
      item.iconPath = new vscode.ThemeIcon("pass", new vscode.ThemeColor("testing.iconPassed"));
    } else if (status === "failed") {
      item.iconPath = new vscode.ThemeIcon("error", new vscode.ThemeColor("testing.iconFailed"));
    } else {
      item.iconPath = vscode.ThemeIcon.File;
    }
    item.tooltip = error ?? node.uri.fsPath;
    item.command = {
      command: "papyrus.scripts.run",
      title: this.mode === "compile" ? "Compile" : "Decompile",
      arguments: [node],
    };
    return item;
  }
}

function insert(root: FolderNode, file: vscode.Uri): void {
  const parts = path.relative(root.uri.fsPath, file.fsPath).split(path.sep);
  let folder = root;
  for (const part of parts.slice(0, -1)) {
    let next = folder.children.find((c): c is FolderNode => c.kind === "folder" && c.name === part);
    if (!next) {
      next = { kind: "folder", uri: vscode.Uri.joinPath(folder.uri, part), name: part, children: [] };
      folder.children.push(next);
    }
    folder = next;
  }
  folder.children.push({ kind: "file", uri: file });
}

/** Folders first, then by name. */
function sort(folder: FolderNode): void {
  const name = (n: FolderNode | FileNode) => (n.kind === "folder" ? n.name : path.basename(n.uri.fsPath)).toLowerCase();
  folder.children.sort((a, b) =>
    a.kind === b.kind ? name(a).localeCompare(name(b)) : a.kind === "folder" ? -1 : 1,
  );
  for (const child of folder.children) {
    if (child.kind === "folder") {
      sort(child);
    }
  }
}

function files(node: Node): vscode.Uri[] {
  switch (node.kind) {
    case "mode":
      return [];
    case "file":
      return [node.uri];
    default:
      return node.children.flatMap(files);
  }
}

/** Registers the view and its commands. */
export function registerScriptsView(context: vscode.ExtensionContext): void {
  const provider = new ScriptsProvider(context.workspaceState);
  const view = vscode.window.createTreeView("papyrus.scripts", { treeDataProvider: provider });
  const updateMode = () => {
    // The section header, in capitals: the most visible text of the view.
    view.title = provider.mode === "compile" ? "Compile" : "Decompile";
    void vscode.commands.executeCommand("setContext", "papyrus.scriptsMode", provider.mode);
  };
  updateMode();

  const decompile = async (uris: vscode.Uri[], open: boolean) => {
    const c = await client();
    if (!c || uris.length === 0) {
      return;
    }
    for (const uri of uris) {
      provider.setStatus(uri.fsPath, "running");
    }
    const result = await c.sendRequest(DecompileFilesRequest, { files: uris.map((u) => u.toString()) });
    let failed = 0;
    for (const r of result.results) {
      const file = vscode.Uri.parse(r.file).fsPath;
      provider.setStatus(file, r.error ? "failed" : "passed", r.error ?? undefined);
      if (r.error) {
        failed += 1;
        state.output?.error(r.error);
      }
    }
    const written = result.results.find((r) => r.psc)?.psc;
    if (open && written) {
      await vscode.window.showTextDocument(vscode.Uri.file(written), { preview: true });
    } else if (!open) {
      const message = `Decompiled ${result.results.length - failed} scripts into ${result.output}`;
      const reveal = "Reveal";
      const choice = await (failed > 0
        ? vscode.window.showWarningMessage(`${message} (${failed} failed, see the output)`, reveal)
        : vscode.window.showInformationMessage(message, reveal));
      if (choice === reveal) {
        await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(result.output));
      }
    }
  };

  const run = async (uris: vscode.Uri[], single: boolean) => {
    if (provider.mode === "compile") {
      await runCompile({ type: "papyrus", files: uris.map((u) => u.fsPath) });
    } else {
      await decompile(uris, single);
    }
  };

  const watcher = vscode.workspace.createFileSystemWatcher("**/*.{psc,pex}", false, true, false);
  let timer: NodeJS.Timeout | undefined;
  const refreshSoon = () => {
    clearTimeout(timer);
    timer = setTimeout(() => provider.refresh(), 300);
  };
  state.statusListeners.push(refreshSoon);

  context.subscriptions.push(
    view,
    watcher,
    watcher.onDidCreate(refreshSoon),
    watcher.onDidDelete(refreshSoon),
    compileEvents.event((e) => provider.setStatus(e.script, e.status)),
    vscode.commands.registerCommand("papyrus.scripts.compileMode", async () => {
      await provider.setMode("compile");
      updateMode();
    }),
    vscode.commands.registerCommand("papyrus.scripts.decompileMode", async () => {
      await provider.setMode("decompile");
      updateMode();
    }),
    vscode.commands.registerCommand("papyrus.scripts.refresh", () => provider.refresh()),
    vscode.commands.registerCommand("papyrus.scripts.run", (node: Node) =>
      run(files(node), node.kind === "file"),
    ),
    vscode.commands.registerCommand("papyrus.scripts.runAll", async () => {
      const roots = await provider.getChildren();
      await run(roots.flatMap(files), false);
    }),
    vscode.commands.registerCommand("papyrus.scripts.open", (node: Node) => {
      if (node.kind !== "file") {
        return;
      }
      if (provider.mode === "compile") {
        void vscode.window.showTextDocument(node.uri);
      } else {
        void vscode.commands.executeCommand("papyrus.decompile", node.uri);
      }
    }),
  );
}
