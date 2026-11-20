// SPDX-License-Identifier: Apache-2.0
// Smoke test of the packaged sysml-wasm module (step 1a.1): load the `wasm-pack build --target web`
// output the way the browser worker will, and call its exports from JavaScript.
//
// Usage: node scripts/smoke_wasm_pkg.mjs [PKG_DIR]   (default: target/wasm-pkg/sysml-wasm)
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const dir = resolve(process.argv[2] ?? "target/wasm-pkg/sysml-wasm");
const pkg = JSON.parse(await readFile(`${dir}/package.json`, "utf8"));
const mod = await import(pathToFileURL(`${dir}/sysml_wasm.js`).href);
await mod.default({ module_or_path: await readFile(`${dir}/sysml_wasm_bg.wasm`) });

const checks = [
  ["version()", mod.version(), pkg.version],
  ["metamodelIri()", mod.metamodelIri(), "https://www.omg.org/spec/SysML/20250201"],
];
let failed = 0;
for (const [call, got, want] of checks) {
  const ok = got === want;
  failed += ok ? 0 : 1;
  console.log(`${ok ? "ok  " : "FAIL"} ${call} = ${JSON.stringify(got)}${ok ? "" : `, expected ${JSON.stringify(want)}`}`);
}
process.exit(failed ? 1 : 0);
