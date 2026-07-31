import { cli, flags } from "@perish/sealkit/cli";
import { init } from "@perish/sealkit/init";
import { io } from "@perish/sealkit/io";

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
flags(args).positionals("init", { allowHelp: true });
if (flags(args).help()) {
  io.print("Usage: runseal :init");
  io.print("");
  io.print("Validate the repository and install versioned git hooks.");
  Deno.exit(0);
}

await init({
  tools: ["git", "tea", "deno", "cargo", "rustc", "ectropy", "plumb", "runseal", "sh"],
  paths: [
    "Cargo.toml",
    "Cargo.lock",
    ".cargo/config.toml",
    "ectropy.toml",
    "plumb.toml",
    "runseal.toml",
    "AGENTS.md",
    "README.md",
    "docs/architecture.md",
    "crates/proto/Cargo.toml",
    "crates/proto/src/lib.rs",
    "crates/runner/Cargo.toml",
    "crates/runner/src/main.rs",
    ".runseal/deno.json",
    ".runseal/deno.lock",
    ".runseal/hooks/pre-commit",
    ".runseal/hooks/commit-msg",
    ".runseal/wrappers/guard.ts",
    ".runseal/wrappers/init.ts",
    ".runseal/wrappers/land.ts",
    ".runseal/wrappers/release.ts",
    ".forgejo/workflows/guard.yml",
    ".forgejo/workflows/release-exact.yml",
    ".forgejo/workflows/release-stable.yml",
  ],
});
