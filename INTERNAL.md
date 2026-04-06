# Cooper — Internal Setup Guide

> Private distribution. Do not share outside the team.

---

## Prerequisites

- **Node.js** 22+ and **npm**
- **GitHub CLI** (`gh`) — [install](https://cli.github.com)
- You must be a member of the **Eldridge-Morgan** GitHub org

---

## 1. One-Time Auth Setup

Authenticate with GitHub Packages:

```bash
gh auth refresh --hostname github.com --scopes read:packages
```

Approve the browser prompt. This adds package read access to your existing GitHub CLI token.

---

## 2. Install the Cooper CLI

### Option A: via npm (recommended for JS developers)

Add this `.npmrc` to your home directory (`~/.npmrc`) or your project root:

```
@eldridge-morgan:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
```

Then install globally:

```bash
GITHUB_TOKEN=$(gh auth token) npm install -g @eldridge-morgan/cooper-cli
```

Verify:

```bash
cooper --help
```

### Option B: via install script

```bash
curl -fsSL https://raw.githubusercontent.com/Eldridge-Morgan/cooper/main/install.sh | GITHUB_TOKEN=$(gh auth token) sh
```

This detects your OS/arch, downloads the correct binary to `~/.cooper/bin/`, and adds it to your PATH.

Open a new terminal (or `source ~/.zshrc`) after install.

### Supported platforms

| OS | Architecture | Status |
|----|-------------|--------|
| macOS | ARM64 (M-series) | Supported |
| macOS | x86_64 (Intel) | Supported |
| Linux | x86_64 | Supported |
| Linux | ARM64 | Supported |

---

## 3. Install the SDK in a project

Every Cooper project needs the TypeScript SDK as a dependency.

Add `.npmrc` to your project root (if not already there):

```
@eldridge-morgan:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
```

Install:

```bash
GITHUB_TOKEN=$(gh auth token) npm install @eldridge-morgan/cooper
```

---

## 4. Create a new project

### From scratch

```bash
cooper new my-app
cd my-app
```

### Manual setup

```
my-app/
├── cooper.config.ts
├── package.json
├── tsconfig.json
├── .npmrc
└── services/
    └── hello.ts
```

**cooper.config.ts**
```ts
export default {
  name: "my-app",
};
```

**package.json**
```json
{
  "name": "my-app",
  "type": "module",
  "scripts": {
    "dev": "cooper run",
    "build": "cooper build"
  },
  "dependencies": {
    "@eldridge-morgan/cooper": "0.1.0"
  },
  "devDependencies": {
    "typescript": "^5.7.0",
    "@types/node": "^22.0.0"
  }
}
```

**services/hello.ts**
```ts
import { api } from "@eldridge-morgan/cooper/api";

export const hello = api(
  { method: "GET", path: "/hello" },
  async () => {
    return { message: "Hello from Cooper!" };
  }
);
```

---

## 5. Run locally

```bash
cooper run
```

This starts:
- Dev server on **http://localhost:4000**
- Dashboard on **http://localhost:9500**
- Embedded Postgres, NATS, and Valkey (auto-provisioned)
- Hot reload on file changes

Test your endpoint:

```bash
curl http://localhost:4000/hello
```

---

## 6. Common commands

| Command | What it does |
|---------|-------------|
| `cooper run` | Start local dev server with hot reload |
| `cooper build` | Production build (bundles TS, generates Dockerfile) |
| `cooper deploy` | Deploy to cloud (AWS, GCP, Azure, Fly.io) |
| `cooper gen client` | Generate typed clients (TS, Python, Rust) |
| `cooper gen openapi` | Generate OpenAPI 3.1 spec |
| `cooper db migrate` | Run database migrations |
| `cooper secrets set KEY` | Set a secret for an environment |
| `cooper logs` | Tail logs from deployed environment |
| `cooper env list` | List environments |

---

## 7. Updating

### Update the CLI

**npm:**
```bash
GITHUB_TOKEN=$(gh auth token) npm update -g @eldridge-morgan/cooper-cli
```

**Install script (re-run it):**
```bash
curl -fsSL https://raw.githubusercontent.com/Eldridge-Morgan/cooper/main/install.sh | GITHUB_TOKEN=$(gh auth token) sh
```

### Update the SDK in a project

```bash
GITHUB_TOKEN=$(gh auth token) npm update @eldridge-morgan/cooper
```

---

## Troubleshooting

### `401 Unauthorized` when installing

Your token doesn't have `read:packages` scope. Run:

```bash
gh auth refresh --hostname github.com --scopes read:packages
```

### `WARN JS workers not started: Cooper bridge not found`

The runtime can't find the SDK bridge. Make sure you've installed `@eldridge-morgan/cooper` in your project and are using Cooper CLI v0.5.1+.

### `No cooper.config.ts found`

Every project needs a `cooper.config.ts` in the root. See section 4.

### Port already in use

Cooper defaults to port 4000. To use a different port:

```bash
cooper run --port 3000
```

---

## Current versions

| Component | Version |
|-----------|---------|
| CLI binary | v0.5.1 |
| SDK | 0.1.0 |

---

*Last updated: 2026-04-06*
