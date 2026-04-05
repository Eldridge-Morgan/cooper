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
    fs::create_dir_all(project_dir.join("migrations"))?;
    fs::create_dir_all(project_dir.join("pages"))?;
    fs::create_dir_all(project_dir.join("islands"))?;
    fs::create_dir_all(project_dir.join("shared"))?;

    // cooper.config.ts
    fs::write(
        project_dir.join("cooper.config.ts"),
        format!(
            r#"export default {{
  name: "{}",
  ssr: {{
    framework: "react",
  }},
}};
"#,
            name
        ),
    )?;

    // package.json
    fs::write(
        project_dir.join("package.json"),
        format!(
            r#"{{
  "name": "{}",
  "private": true,
  "scripts": {{
    "dev": "cooper run",
    "build": "cooper build",
    "deploy": "cooper deploy"
  }},
  "dependencies": {{
    "cooper": "latest",
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

    // Example service
    fs::write(
        project_dir.join("services/users/api.ts"),
        r#"import { api } from "cooper/api";
import { database } from "cooper/db";
import { z } from "zod";

export const db = database("main", {
  migrations: "./migrations",
});

const CreateUserSchema = z.object({
  name: z.string().min(1).max(100),
  email: z.string().email(),
});

export const listUsers = api(
  { method: "GET", path: "/users" },
  async () => {
    const users = await db.query("SELECT * FROM users");
    return { users };
  }
);

export const getUser = api(
  { method: "GET", path: "/users/:id", auth: true },
  async ({ id }: { id: string }, principal) => {
    const user = await db.queryRow("SELECT * FROM users WHERE id = $1", [id]);
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
    return { user };
  }
);
"#,
    )?;

    // Example migration
    fs::write(
        project_dir.join("migrations/001_users.sql"),
        r#"CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
"#,
    )?;

    // Example page
    fs::write(
        project_dir.join("pages/index.tsx"),
        r#"import { page } from "cooper/ssr";

export default page(async () => {
  return (
    <div>
      <h1>Welcome to Cooper</h1>
      <p>Edit pages/index.tsx to get started.</p>
    </div>
  );
});
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
    eprintln!("  Next steps:");
    eprintln!("    {} {}", "cd".bold(), name);
    eprintln!("    {}", "cooper run".bold());
    eprintln!();

    Ok(())
}
