# Database

Declare databases in code. Cooper starts embedded Postgres locally and provisions managed databases on deploy.

## Declare

```ts
import { database } from "cooper/db";

export const db = database("main", {
  engine: "postgres",       // "postgres" | "mysql"
  migrations: "./migrations",
});
```

## Query

```ts
// Multiple rows
const users = await db.query<User>("SELECT * FROM users WHERE active = $1", [true]);

// Single row
const user = await db.queryRow<User>("SELECT * FROM users WHERE id = $1", [id]);

// Execute (INSERT, UPDATE, DELETE)
const { rowCount } = await db.exec("DELETE FROM sessions WHERE expires_at < NOW()");

// Insert helper
const user = await db.insert<User>("users", { name: "Alice", email: "a@b.com" });
```

## Migrations

SQL files in order:

```
migrations/
  001_users.sql
  002_posts.sql
  003_add_index.sql
```

Cooper runs these automatically on `cooper run` and `cooper deploy`.

## ORM support

Cooper provides the connection — use any ORM:

```ts
import { drizzle } from "drizzle-orm/node-postgres";

export const db = database("main", { migrations: "./migrations" });
export const orm = drizzle(db.pool);

const users = await orm.select().from(usersTable).where(eq(usersTable.email, email));
```

## Engines

| Engine | Local | AWS | GCP | Azure | Fly |
|---|---|---|---|---|---|
| `postgres` | Embedded | RDS | Cloud SQL | Azure DB | Fly Postgres |
| `mysql` | Embedded | RDS | Cloud SQL | Azure DB | — |
