import { copyFile, mkdir, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const skillSource = join(root, "crates", "cli", "assets", "stormbuffer-memory.md");
const verifierSource = join(root, ".agents", "skills", "stormbuffer-memory", "verify.py");
const targets = [
	join(root, "packages", "codex-plugin-stormbuffer", "skills", "stormbuffer-memory"),
	join(root, "packages", "pi-plugin-stormbuffer", "skills", "stormbuffer-memory"),
];

for (const target of targets) {
	await rm(target, { recursive: true, force: true });
	await mkdir(target, { recursive: true });
	await copyFile(skillSource, join(target, "SKILL.md"));
	await copyFile(verifierSource, join(target, "verify.py"));
}
