import { RequestType, RequestType0 } from "vscode-languageclient/node";

/** Result of the `papyrus/status` and `papyrus/restart` requests. */
export interface ServerStatus {
  files: number;
  openFiles: number;
  indexing: boolean;
  generation: number;
  roots: string[];
  importDirectories: string[];
  health?: "ok" | "limited" | "warning" | "error";
  problems?: string[];
  manifest?: string | null;
  manifestError?: string | null;
  sources?: string[];
  flagsFile?: string | null;
  variables?: Record<string, string>;
}

/** One compiler invocation of `papyrus/compileCommands`. */
export interface CompileCommand {
  program: string;
  args: string[];
  cwd: string;
  output: string;
  commandLine: string;
}

/** Result of `papyrus/compileCommands`. */
export interface CompileCommands {
  manifest: string;
  output: string;
  commands: CompileCommand[];
}

/**
 * `papyrus/restart`: the server drops its indexes, applies `settings`,
 * re-indexes the workspace and republishes diagnostics. Implemented on the
 * server side because an activated extension cannot restart itself.
 */
export const RestartRequest = new RequestType<{ settings?: unknown }, ServerStatus, void>("papyrus/restart");

/** `papyrus/status`: what the server currently knows. */
export const StatusRequest = new RequestType0<ServerStatus, void>("papyrus/status");

/** `papyrus/syntaxTree`: a debug dump of the syntax tree of a document. */
export const SyntaxTreeRequest = new RequestType<{ textDocument: { uri: string } }, string, void>(
  "papyrus/syntaxTree",
);

/** `papyrus/compileCommands`: the compiler invocations of `papyrus.toml`. */
export const CompileCommandsRequest = new RequestType<
  { textDocument?: { uri: string }; files?: { uri: string }[]; all: boolean },
  CompileCommands,
  void
>("papyrus/compileCommands");

/** `papyrus/manifestTemplate`: a commented `papyrus.toml` to start from. */
export const ManifestTemplateRequest = new RequestType0<string, void>("papyrus/manifestTemplate");
