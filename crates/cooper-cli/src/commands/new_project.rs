use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

pub async fn run(name: &str, _template: &str) -> Result<()> {
    let project_dir = Path::new(name);

    if project_dir.exists() {
        return Err(anyhow::anyhow!("Directory '{}' already exists", name));
    }

    eprintln!(
        "  {} Creating project '{}'...\n",
        "→".cyan(),
        name.bold()
    );

    // Create directory structure
    fs::create_dir_all(project_dir.join("services/users"))?;
    fs::create_dir_all(project_dir.join("services/health"))?;
    fs::create_dir_all(project_dir.join("migrations"))?;
    fs::create_dir_all(project_dir.join("pages"))?;
    fs::create_dir_all(project_dir.join("islands"))?;
    fs::create_dir_all(project_dir.join("shared"))?;

    // cooper.config.ts
    fs::write(
        project_dir.join("cooper.config.ts"),
        format!(
            r#"import {{ secret }} from "cooper-stack/secrets";

export default {{
  name: "{}",
  ssr: {{
    framework: "react",
    assets: {{
      cdn: true,
      compress: true,
    }},
  }},
  observability: {{
    // traces: {{ provider: "datadog", apiKey: secret("dd-key") }},
    // metrics: {{ provider: "grafana", endpoint: secret("grafana-url") }},
  }},
  docs: {{
    title: "{} API",
    description: "API documentation",
  }},
}};
"#,
            name, name
        ),
    )?;

    // package.json
    fs::write(
        project_dir.join("package.json"),
        format!(
            r#"{{
  "name": "{}",
  "private": true,
  "type": "module",
  "scripts": {{
    "dev": "cooper run",
    "build": "cooper build",
    "deploy": "cooper deploy"
  }},
  "dependencies": {{
    "cooper-stack": "latest",
    "zod": "^3.23"
  }},
  "devDependencies": {{
    "typescript": "^5.7",
    "@types/node": "^22"
  }}
}}
"#,
            name
        ),
    )?;

    // tsconfig.json
    fs::write(
        project_dir.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "dist",
    "rootDir": ".",
    "jsx": "react-jsx",
    "paths": {
      "~/*": ["./*"],
      "~gen/*": [".cooper/gen/*"]
    }
  },
  "include": ["**/*.ts", "**/*.tsx"],
  "exclude": ["node_modules", "dist"]
}
"#,
    )?;

    // Users service — full example with CRUD, auth, validation, middleware
    fs::write(
        project_dir.join("services/users/api.ts"),
        r#"import { api } from "cooper-stack/api";
import { CooperError } from "cooper-stack";
import { database } from "cooper-stack/db";
import { cache } from "cooper-stack/cache";
import { topic } from "cooper-stack/pubsub";
import { z } from "zod";

// Database
export const db = database("main", {
  migrations: "./migrations",
});

// Cache
const userCache = cache<any>("users", { ttl: "5m" });

// Events
export const UserCreated = topic<{ userId: string; email: string }>(
  "user-created",
  { deliveryGuarantee: "at-least-once" }
);

// Validation schemas
const CreateUserSchema = z.object({
  name: z.string().min(1).max(100),
  email: z.string().email(),
});

const UpdateUserSchema = z.object({
  name: z.string().min(1).max(100).optional(),
  email: z.string().email().optional(),
});

// Routes
export const listUsers = api(
  { method: "GET", path: "/users" },
  async () => {
    const users = await db.query("SELECT id, name, email, created_at FROM users ORDER BY created_at DESC");
    return { users };
  }
);

export const getUser = api(
  { method: "GET", path: "/users/:id" },
  async ({ id }: { id: string }) => {
    const user = await userCache.getOrSet(id, async () => {
      return db.queryRow("SELECT * FROM users WHERE id = $1", [id]);
    });
    if (!user) throw new CooperError("NOT_FOUND", `User ${id} not found`);
    return { user };
  }
);

export const createUser = api(
  { method: "POST", path: "/users", validate: CreateUserSchema },
  async (req) => {
    const user = await db.queryRow(
      "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING *",
      [req.name, req.email]
    );
    // Publish event
    await UserCreated.publish({ userId: user.id, email: user.email });
    return { user };
  }
);

export const updateUser = api(
  { method: "PATCH", path: "/users/:id", auth: true, validate: UpdateUserSchema },
  async ({ id, ...updates }: { id: string; name?: string; email?: string }, principal) => {
    const sets: string[] = [];
    const values: any[] = [];
    let idx = 1;

    if (updates.name) { sets.push(`name = $${idx++}`); values.push(updates.name); }
    if (updates.email) { sets.push(`email = $${idx++}`); values.push(updates.email); }

    if (sets.length === 0) return { user: await db.queryRow("SELECT * FROM users WHERE id = $1", [id]) };

    values.push(id);
    const user = await db.queryRow(
      `UPDATE users SET ${sets.join(", ")} WHERE id = $${idx} RETURNING *`,
      values
    );
    if (!user) throw new CooperError("NOT_FOUND", `User ${id} not found`);

    await userCache.delete(id);
    return { user };
  }
);

export const deleteUser = api(
  { method: "DELETE", path: "/users/:id", auth: true },
  async ({ id }: { id: string }) => {
    const result = await db.exec("DELETE FROM users WHERE id = $1", [id]);
    if (result.rowCount === 0) throw new CooperError("NOT_FOUND", `User ${id} not found`);
    await userCache.delete(id);
    return { deleted: true };
  }
);
"#,
    )?;

    // Health service
    fs::write(
        project_dir.join("services/health/api.ts"),
        r#"import { api } from "cooper-stack/api";

export const healthCheck = api(
  { method: "GET", path: "/health" },
  async () => {
    return {
      status: "ok",
      timestamp: new Date().toISOString(),
      uptime: process.uptime(),
    };
  }
);
"#,
    )?;

    // Auth handler
    fs::write(
        project_dir.join("services/auth.ts"),
        r#"import { authHandler } from "cooper-stack/auth";
import { api } from "cooper-stack/api";
import { CooperError } from "cooper-stack";

// Register the auth handler — called for every route with auth: true
export const auth = authHandler(async (token: string) => {
  // Replace with your actual JWT verification
  // For now, accept any non-empty token
  if (!token || token === "invalid") {
    throw new CooperError("UNAUTHORIZED", "Invalid token");
  }

  // Return the principal — injected into protected route handlers
  return {
    userId: "user_from_token",
    role: "user",
  };
});
"#,
    )?;

    // Event subscribers
    fs::write(
        project_dir.join("services/users/events.ts"),
        r#"import { UserCreated } from "./api";

// Send welcome email when a user signs up
export const onUserCreated = UserCreated.subscribe("send-welcome", {
  concurrency: 5,
  handler: async ({ userId, email }) => {
    console.log(`[event] Welcome email would be sent to ${email} (userId: ${userId})`);
  },
});
"#,
    )?;

    // Cron job example
    fs::write(
        project_dir.join("services/cleanup.ts"),
        r#"import { cron } from "cooper-stack/cron";

export const sessionCleanup = cron("session-cleanup", {
  schedule: "every 1 hour",
  handler: async () => {
    console.log("[cron] Session cleanup would run here");
  },
});
"#,
    )?;

    // Migration
    fs::write(
        project_dir.join("migrations/001_users.sql"),
        r#"CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
"#,
    )?;

    // Example page
    fs::write(
        project_dir.join("pages/index.tsx"),
        r#"import { page } from "cooper-stack/ssr";

export default page(async () => {
  return (
    <div>
      <h1>Welcome to Cooper</h1>
      <p>Your backend is running. Try these endpoints:</p>
      <ul>
        <li><code>GET /health</code> — health check</li>
        <li><code>GET /users</code> — list users</li>
        <li><code>POST /users</code> — create a user</li>
        <li><code>GET /users/:id</code> — get a user</li>
        <li><code>GET /_cooper/info</code> — API info</li>
      </ul>
    </div>
  );
});
"#,
    )?;

    // Shared types
    fs::write(
        project_dir.join("shared/types.ts"),
        r#"export interface User {
  id: string;
  name: string;
  email: string;
  created_at: string;
  updated_at: string;
}
"#,
    )?;

    // .gitignore
    fs::write(
        project_dir.join(".gitignore"),
        r#"node_modules/
dist/
.cooper/
*.env
*.env.local
"#,
    )?;

    eprintln!("  {} Created project structure", "✓".green());
    eprintln!();
    eprintln!(
        "  {} {} {}",
        "📁".to_string(),
        "services/users/api.ts".dimmed(),
        "— CRUD with validation, cache, events"
    );
    eprintln!(
        "  {} {} {}",
        "📁".to_string(),
        "services/auth.ts".dimmed(),
        "— JWT auth handler"
    );
    eprintln!(
        "  {} {} {}",
        "📁".to_string(),
        "services/cleanup.ts".dimmed(),
        "— cron job example"
    );
    eprintln!(
        "  {} {} {}",
        "📁".to_string(),
        "pages/index.tsx".dimmed(),
        "— SSR page"
    );
    eprintln!();
    eprintln!("  Next steps:");
    eprintln!("    {} {}", "cd".bold(), name);
    eprintln!("    {}", "cooper run".bold());
    eprintln!();

    Ok(())
}
