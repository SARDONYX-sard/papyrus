import type * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

import { isWithin } from "./paths";
import type { BuildDirs } from "./protocol";

/**
 * State shared by the modules of the extension. Modules read it instead of
 * importing each other, which keeps the dependencies one-directional.
 */
export const state: {
  context?: vscode.ExtensionContext;
  output?: vscode.LogOutputChannel;
  client?: LanguageClient;
  /** A `papyrus.toml` was found: compiling is available. */
  hasManifest: boolean;
  /** The `papyrus.toml` of the workspace. */
  manifest?: string;
  /** `[build] sources` of `papyrus.toml`. */
  sources: string[];
  /** `[test] sources` of `papyrus.toml`. */
  testSources: string[];
  /** The output directories of each workspace folder. */
  buildDirs: BuildDirs[];
  /** Called when the sources or the output directories change. */
  statusListeners: (() => void)[];
  /** The last status of the server, as the explorer shows it. */
  server: ServerSummary;
  /** Called after each status update of the server. */
  serverListeners: (() => void)[];
} = {
  hasManifest: false,
  sources: [],
  testSources: [],
  buildDirs: [],
  statusListeners: [],
  server: { label: "Papyrus: starting", icon: "loading~spin", tooltip: "Starting the language server" },
  serverListeners: [],
};

/** The server status, ready to show: a codicon and its theme color, a text, a Markdown tooltip. */
export interface ServerSummary {
  label: string;
  icon: string;
  color?: string;
  tooltip: string;
}

/**
 * How the server treats the script `file`: a script of `[build] sources` or
 * of `[test] sources`, or `limited`: outside of every project (or without
 * papyrus.toml), checked only while open and compiled with the settings of
 * the first project, or the built-in ones.
 */
export function scriptScope(file: string): "sources" | "tests" | "limited" {
  if (state.testSources.some((dir) => isWithin(file, dir))) {
    return "tests";
  }
  return state.sources.some((dir) => isWithin(file, dir)) ? "sources" : "limited";
}

/** The client, when the server is running. */
export function runningClient(): LanguageClient | undefined {
  return state.client?.isRunning() ? state.client : undefined;
}

/** Logs to the output channel of the extension. */
export function log(message: string): void {
  state.output?.appendLine(message);
}
