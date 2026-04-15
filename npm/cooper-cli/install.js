#!/usr/bin/env node

// Resolves the correct platform-specific binary package and symlinks it.
// This is the same pattern used by esbuild, turbo, and Prisma.

const { existsSync, mkdirSync, copyFileSync, chmodSync } = require("fs");
const { join } = require("path");

const PLATFORMS = {
  "darwin-arm64": "@eldridge-morgan/cooper-darwin-arm64",
  "darwin-x64": "@eldridge-morgan/cooper-darwin-x64",
  "linux-x64": "@eldridge-morgan/cooper-linux-x64",
  "linux-arm64": "@eldridge-morgan/cooper-linux-arm64",
  "win32-x64": "@eldridge-morgan/cooper-win32-x64",
};

function main() {
  const platform = process.platform;
  const arch = process.arch;
  const key = `${platform}-${arch}`;
  const pkg = PLATFORMS[key];

  if (!pkg) {
    console.error(
      `Cooper does not have a prebuilt binary for ${platform}-${arch}.\n` +
        `Supported: ${Object.keys(PLATFORMS).join(", ")}`
    );
    process.exit(1);
  }

  let binaryPath;
  try {
    const pkgDir = require.resolve(`${pkg}/package.json`);
    const dir = join(pkgDir, "..");
    const binFile = platform === "win32" ? "cooper.exe" : "cooper";
    binaryPath = join(dir, "bin", binFile);
  } catch {
    console.error(
      `Failed to find package ${pkg}.\n` +
        `Make sure you have configured the GitHub Packages registry:\n\n` +
        `  npm login --registry=https://npm.pkg.github.com --scope=@eldridge-morgan\n`
    );
    process.exit(1);
  }

  if (!existsSync(binaryPath)) {
    console.error(`Binary not found at ${binaryPath}`);
    process.exit(1);
  }

  const binDir = join(__dirname, "bin");
  mkdirSync(binDir, { recursive: true });

  const isWindows = platform === "win32";
  const binName = isWindows ? "cooper.exe" : "cooper";
  const dest = join(binDir, binName);
  copyFileSync(binaryPath, dest);
  chmodSync(dest, 0o755);

  console.log(`✓ Cooper CLI installed (${key})`);
}

main();
