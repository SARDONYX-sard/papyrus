import * as path from "node:path";
import * as vscode from "vscode";

import { compileEvents, runCompile } from "./compile/task";
import { client } from "./decompile";
import { DecompileFilesRequest } from "./protocol";
import { state } from "./state";

/**
 * The Papyrus scripts view: the `.psc` and `.pex` files of the workspace as
 * one tree. The mode, switched by a click on the first row, chooses which of
 * them the view acts on:
 *
 * - compile: the `.psc` sources (`[build] sources` of papyrus.toml, else the
 *   workspace folders), outside of the build directories;
 * - decompile: the `.pex` files.
 *
 * The other files stay visible, dimmed like ignored files, so that outputs
 * show up next to their sources as they are written; files written since the
 * last run get a dot. Below the mode, the compiler (compile mode) and the
 * output directory of the mode.
 *
 * A click opens a file; the ▶ button of a file or folder runs the
 * action on it (everything below, for a folder). Each script shows the
 * result of its last run. Outputs keep the directory structure:
 * `[build] output` of papyrus.toml (`decompile` below it), else
 * `build/papyrus/compile` and `build/papyrus/decompile` of the workspace
 * folder.
 */

type Mode = "compile" | "decompile";
type Status = "running" | "passed" | "failed";

const MODE_KEY = "papyrus.scriptsView.mode";
/** The scheme of the rows, so that their decorations stay in this view. */
const SCHEME = "papyrus-scripts";

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

/** In compile mode: the compiler used, built-in or `[compiler] path`. */
interface CompilerNode {
  kind: "compiler";
}

/** Where the mode writes its files; a click reveals the directory. */
interface OutputNode {
  kind: "output";
}

type Node = ModeNode | CompilerNode | OutputNode | FolderNode | FileNode;

/** Path comparison key; Windows paths ignore case. */
function key(p: string): string {
  const n = path.normalize(p);
  return process.platform === "win32" ? n.toLowerCase() : n;
}

function isBelow(file: string, dir: string): boolean {
  const rel = path.relative(key(dir), key(file));
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

function isWithin(file: string, dir: string): boolean {
  return key(file) === key(dir) || isBelow(file, dir);
}

/** The row URI of a file: same path, own scheme. */
function rowUri(uri: vscode.Uri): vscode.Uri {
  return uri.with({ scheme: SCHEME });
}

class ScriptsProvider implements vscode.TreeDataProvider<Node>, vscode.FileDecorationProvider {
  private readonly changed = new vscode.EventEmitter<Node | undefined>();
  readonly onDidChangeTreeData = this.changed.event;
  private readonly decorationsChanged = new vscode.EventEmitter<vscode.Uri | vscode.Uri[] | undefined>();
  readonly onDidChangeFileDecorations = this.decorationsChanged.event;
  mode: Mode;
  private roots: Node[] = [];
  private loaded = false;
  private readonly statuses = new Map<string, Status>();
  private readonly errors = new Map<string, string>();
  /** Files written since the last run started, by path key. */
  private readonly fresh = new Set<string>();
  /** The file nodes by path key, to refresh one row when its status changes. */
  private nodes = new Map<string, FileNode>();
  /**
   * Part of the folder ids: VS Code keeps the expansion of known ids, so
   * new ids are what applies `expanded` to every folder.
   */
  private generation = 0;
  private expanded = false;

  constructor(private readonly memento: vscode.Memento) {
    this.mode = memento.get<Mode>(MODE_KEY, "compile");
  }

  async setMode(mode: Mode): Promise<void> {
    this.mode = mode;
    await this.memento.update(MODE_KEY, mode);
    // Same files, other targets.
    this.changed.fire(undefined);
    this.decorationsChanged.fire(undefined);
  }

  refresh(): void {
    this.loaded = false;
    this.changed.fire(undefined);
    this.decorationsChanged.fire(undefined);
  }

  setStatus(file: string, status: Status, error?: string): void {
    this.statuses.set(key(file), status);
    if (error) {
      this.errors.set(key(file), error);
    } else {
      this.errors.delete(key(file));
    }
    // Only the row of the file: redrawing thousands of rows for each script
    // compiled keeps the extension host busy.
    const node = this.nodes.get(key(file));
    if (node) {
      this.changed.fire(node);
    }
  }

  /** Expands every folder. Collapsing is the built-in button of the view. */
  expandAll(): void {
    this.generation += 1;
    this.expanded = true;
    this.changed.fire(undefined);
  }

  /** A file was written on disk. */
  markFresh(uri: vscode.Uri): void {
    this.fresh.add(key(uri.fsPath));
    this.decorationsChanged.fire(rowUri(uri));
  }

  /** A run starts: earlier outputs are not new anymore. */
  clearFresh(): void {
    if (this.fresh.size > 0) {
      this.fresh.clear();
      this.decorationsChanged.fire(undefined);
    }
  }

  /** The directories that get compiled or decompiled files. */
  private buildDirs(): string[] {
    const folders = (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
    return [
      ...state.buildDirs.flatMap((b) => [b.compile, b.decompile]),
      ...folders.map((f) => path.join(f, "build", "papyrus")),
    ];
  }

  /** The output directories of the current mode. */
  outputDirs(): string[] {
    const dirs = state.buildDirs.map((b) => (this.mode === "compile" ? b.compile : b.decompile));
    return [...new Map(dirs.map((d) => [key(d), d])).values()];
  }

  /** Whether the current mode acts on `file`. */
  isTarget(file: string): boolean {
    if (this.mode === "decompile") {
      return file.toLowerCase().endsWith(".pex");
    }
    if (!file.toLowerCase().endsWith(".psc")) {
      return false;
    }
    // Decompiled scripts are not sources.
    if (this.buildDirs().some((dir) => isBelow(file, dir))) {
      return false;
    }
    const folders = (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
    const sources = state.sources.length > 0 ? state.sources : folders;
    return sources.some((dir) => isWithin(file, dir));
  }

  /** The workspace folders, plus the sources and outputs outside of them. */
  private rootDirs(): string[] {
    const roots: string[] = [];
    const folders = (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
    const outputs = state.buildDirs.flatMap((b) => [b.compile, b.decompile]);
    for (const dir of [...folders, ...state.sources, ...outputs]) {
      if (!roots.some((r) => isWithin(dir, r))) {
        roots.push(dir);
      }
    }
    return roots;
  }

  private async load(): Promise<void> {
    const include = this.rootDirs();
    // Excludes are relative to each root: outputs below `target` (e.g.
    // `${workspaceFolder}/target/...`) are found from their own root.
    const found = await Promise.all(
      include.map((dir) =>
        vscode.workspace.findFiles(
          new vscode.RelativePattern(vscode.Uri.file(dir), "**/*.{psc,pex}"),
          new vscode.RelativePattern(vscode.Uri.file(dir), "**/{node_modules,target,.git}/**"),
        ),
      ),
    );
    const roots: FolderNode[] = include.map((dir) => ({
      kind: "folder",
      uri: vscode.Uri.file(dir),
      name: path.basename(dir),
      children: [],
    }));
    const nodes = new Map<string, FileNode>();
    found.forEach((files, i) => {
      for (const file of files) {
        const k = key(file.fsPath);
        if (!nodes.has(k)) {
          nodes.set(k, insert(roots[i], file));
        }
      }
    });
    this.nodes = nodes;
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
      const header: Node[] =
        this.mode === "compile"
          ? [{ kind: "mode" }, { kind: "compiler" }, { kind: "output" }]
          : [{ kind: "mode" }, { kind: "output" }];
      return [...header, ...this.roots];
    }
    return node.kind === "folder" ? node.children : [];
  }

  /** The files of `node` the current mode acts on. */
  targets(node: Node): vscode.Uri[] {
    switch (node.kind) {
      case "file":
        return this.isTarget(node.uri.fsPath) ? [node.uri] : [];
      case "folder":
        return node.children.flatMap((c) => this.targets(c));
      default:
        return [];
    }
  }

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== SCHEME) {
      return undefined;
    }
    const file = uri.with({ scheme: "file" });
    const node = this.nodes.get(key(file.fsPath));
    const folder = !node;
    const active = folder ? this.folderHasTargets(file.fsPath) : this.isTarget(file.fsPath);
    const fresh = !folder && this.fresh.has(key(file.fsPath));
    if (active && !fresh) {
      return undefined;
    }
    return {
      badge: fresh ? "•" : undefined,
      tooltip: fresh ? "Written since the last run" : undefined,
      color: active ? undefined : new vscode.ThemeColor("gitDecoration.ignoredResourceForeground"),
    };
  }

  private folderHasTargets(dir: string): boolean {
    for (const node of this.nodes.values()) {
      if (isBelow(node.uri.fsPath, dir) && this.isTarget(node.uri.fsPath)) {
        return true;
      }
    }
    return false;
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
        ? "Compile mode: ▶ compiles the .psc sources. Click here to switch to decompile mode."
        : "Decompile mode: ▶ decompiles the .pex files. Click here to switch to compile mode.";
      item.command = {
        command: compile ? "papyrus.scripts.decompileMode" : "papyrus.scripts.compileMode",
        title: "Switch mode",
      };
      return item;
    }
    if (node.kind === "compiler") {
      return compilerItem();
    }
    if (node.kind === "output") {
      return this.outputItem();
    }
    if (node.kind === "folder") {
      // Collapsed until "Expand All": a workspace may hold thousands of scripts.
      const item = new vscode.TreeItem(
        node.name,
        this.expanded ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed,
      );
      item.id = `${this.generation}:${key(node.uri.fsPath)}`;
      item.resourceUri = rowUri(node.uri);
      item.contextValue = this.targets(node).length > 0 ? "papyrusFolder" : "papyrusFolderOther";
      item.iconPath = vscode.ThemeIcon.Folder;
      item.tooltip = node.uri.fsPath;
      return item;
    }
    const item = new vscode.TreeItem(rowUri(node.uri), vscode.TreeItemCollapsibleState.None);
    item.contextValue = this.isTarget(node.uri.fsPath) ? "papyrusFile" : "papyrusFileOther";
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
    item.command = { command: "papyrus.scripts.open", title: "Open", arguments: [node] };
    return item;
  }

  private outputItem(): vscode.TreeItem {
    const dirs = this.outputDirs();
    const folders = (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
    const show = (dir: string) => {
      const folder = folders.find((f) => isWithin(dir, f));
      return folder ? path.relative(folder, dir) || "." : dir;
    };
    const item = new vscode.TreeItem(
      `Output: ${dirs.length === 1 ? show(dirs[0]) : dirs.length === 0 ? "—" : `${dirs.length} folders`}`,
    );
    item.contextValue = "papyrusOutput";
    item.iconPath = new vscode.ThemeIcon("folder-opened");
    item.tooltip = dirs.length > 0 ? `${dirs.join("\n")}\n\nClick to reveal.` : "No output directory yet.";
    if (dirs.length > 0) {
      item.command = {
        command: "revealFileInOS",
        title: "Reveal",
        arguments: [vscode.Uri.file(dirs[0])],
      };
    }
    return item;
  }
}

/** The compiler row: the one of every project, or `mixed` when they differ. */
function compilerItem(): vscode.TreeItem {
  const builtin = "built-in";
  const compilers = [...new Set(state.buildDirs.map((b) => b.compiler ?? builtin))];
  const only = compilers.length === 1 ? compilers[0] : undefined;
  const label =
    compilers.length === 0 ? builtin : only === undefined ? "mixed" : only === builtin ? builtin : path.basename(only);
  const item = new vscode.TreeItem(`Compiler: ${label}`);
  item.contextValue = "papyrusCompiler";
  item.iconPath = new vscode.ThemeIcon(only === builtin || compilers.length === 0 ? "package" : "tools");
  if (only !== undefined && only !== builtin) {
    item.description = path.dirname(only);
  } else if (only === builtin || compilers.length === 0) {
    item.description = state.hasManifest ? "no [compiler] path" : "no papyrus.toml";
  }
  const lines = state.buildDirs.map((b) => `${b.root}: ${b.compiler ?? builtin}`);
  item.tooltip =
    lines.length > 0
      ? `${lines.join("\n")}\n\nSet [compiler] path in papyrus.toml to use PapyrusCompiler.exe.`
      : "The built-in compiler of papyrus_analyzer.";
  return item;
}

function insert(root: FolderNode, file: vscode.Uri): FileNode {
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
  const node: FileNode = { kind: "file", uri: file };
  folder.children.push(node);
  return node;
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

/** Opens a script, or the decompiled source of a `.pex` file. */
function open(uri: vscode.Uri): void {
  if (uri.fsPath.toLowerCase().endsWith(".pex")) {
    void vscode.commands.executeCommand("papyrus.decompile", uri);
  } else {
    void vscode.window.showTextDocument(uri);
  }
}

/** Registers the view and its commands. */
export function registerScriptsView(context: vscode.ExtensionContext): void {
  const provider = new ScriptsProvider(context.workspaceState);
  const view = vscode.window.createTreeView("papyrus.scripts", {
    treeDataProvider: provider,
    showCollapseAll: true,
  });
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
    if (uris.length === 0) {
      return;
    }
    provider.clearFresh();
    if (provider.mode === "compile") {
      await runCompile({ type: "papyrus", files: uris.map((u) => u.fsPath) });
    } else {
      await decompile(uris, single);
    }
  };

  const watcher = vscode.workspace.createFileSystemWatcher("**/*.{psc,pex}");
  let timer: NodeJS.Timeout | undefined;
  const refreshSoon = () => {
    clearTimeout(timer);
    timer = setTimeout(() => provider.refresh(), 300);
  };
  state.statusListeners.push(refreshSoon);

  context.subscriptions.push(
    view,
    watcher,
    vscode.window.registerFileDecorationProvider(provider),
    watcher.onDidCreate((uri) => {
      provider.markFresh(uri);
      refreshSoon();
    }),
    watcher.onDidChange((uri) => provider.markFresh(uri)),
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
    vscode.commands.registerCommand("papyrus.scripts.expandAll", () => provider.expandAll()),
    vscode.commands.registerCommand("papyrus.scripts.run", (node: Node) =>
      run(provider.targets(node), node.kind === "file"),
    ),
    vscode.commands.registerCommand("papyrus.scripts.runAll", async () => {
      const roots = await provider.getChildren();
      await run(
        roots.flatMap((r) => provider.targets(r)),
        false,
      );
    }),
    vscode.commands.registerCommand("papyrus.scripts.open", (node: Node) => {
      if (node.kind === "file") {
        open(node.uri);
      }
    }),
  );
}
