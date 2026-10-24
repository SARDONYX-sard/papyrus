import type * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

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
  /** `[build] sources` of `papyrus.toml`. */
  sources: string[];
  /** The output directories of each workspace folder. */
  buildDirs: BuildDirs[];
  /** Called after each status update of the server. */
  statusListeners: (() => void)[];
} = {
  hasManifest: false,
  sources: [],
  buildDirs: [],
  statusListeners: [],
};

/** The client, when the server is running. */
export function runningClient(): LanguageClient | undefined {
  return state.client?.isRunning() ? state.client : undefined;
}

/** Logs to the output channel of the extension. */
export function log(message: string): void {
  state.output?.appendLine(message);
}
