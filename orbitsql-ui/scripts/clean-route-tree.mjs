import { readFile, writeFile } from "node:fs/promises";

const path = new URL("../src/routeTree.gen.ts", import.meta.url);
const source = await readFile(path, "utf8");
const firstImport = source.indexOf("import ");
const cleaned = firstImport >= 0 ? source.slice(firstImport) : source;
await writeFile(path, cleaned);
