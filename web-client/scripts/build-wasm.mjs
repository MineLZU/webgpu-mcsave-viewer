// Builds crates/viewer-web for wasm32 and writes its wasm-bindgen ES module into src/wasm/.
// `--release` builds optimised; the default is a debug build with assertions.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const release = process.argv.includes("--release");
const root = path.resolve(fileURLToPath(new URL(".", import.meta.url)), "../..");
const profile = release ? "release" : "debug";

const run = (command, args) => execFileSync(command, args, { cwd: root, stdio: "inherit" });

run("cargo", ["build", "-p", "viewer-web", "--target", "wasm32-unknown-unknown", ...(release ? ["--release"] : [])]);
run("wasm-bindgen", [
  path.join(root, "target", "wasm32-unknown-unknown", profile, "viewer_web.wasm"),
  "--target", "web",
  "--out-dir", path.join(root, "web-client", "src", "wasm"),
  "--out-name", "viewer_web",
]);
