import * as path from "node:path";

/** Path comparison key; Windows paths ignore case. */
export function key(p: string): string {
  const n = path.normalize(p);
  return process.platform === "win32" ? n.toLowerCase() : n;
}

export function isBelow(file: string, dir: string): boolean {
  const rel = path.relative(key(dir), key(file));
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

export function isWithin(file: string, dir: string): boolean {
  return key(file) === key(dir) || isBelow(file, dir);
}
