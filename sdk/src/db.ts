import { registry } from "./registry.js";

export interface DatabaseConfig {
  engine?: "postgres" | "mysql" | "mongodb" | "dynamodb";
  migrations?: string;
  partitionKey?: string;
  sortKey?: string;
}

export interface DatabaseClient {
  /** Run a query returning multiple rows */
  query<T = any>(sql: string, params?: any[]): Promise<T[]>;
  /** Run a query returning a single row or null */
  queryRow<T = any>(sql: string, params?: any[]): Promise<T | null>;
  /** Run a query returning affected row count */
  exec(sql: string, params?: any[]): Promise<{ rowCount: number }>;
  /** Insert and return the inserted row */
  insert<T = any>(table: string, data: Record<string, any>): Promise<T>;
  /** Access the underlying connection pool (for ORMs like Drizzle) */
  pool: any;
}

/**
 * Declare a database.
 *
 * ```ts
 * export const db = database("main", { migrations: "./migrations" });
 * const user = await db.queryRow<User>("SELECT * FROM users WHERE id = $1", [id]);
 * ```
 */
export function database(name: string, config?: DatabaseConfig): DatabaseClient {
  const engine = config?.engine ?? "postgres";

  // Connection details injected by the Rust runtime via env vars
  const connStr = process.env[`COOPER_DB_${name.toUpperCase()}_URL`]
    ?? `postgres://cooper:cooper@localhost:5432/cooper_${name}`;

  let pool: any = null;

  const ensurePool = async () => {
    if (pool) return pool;

    if (engine === "postgres") {
      const pg = await import("pg");
      pool = new pg.default.Pool({ connectionString: connStr });
      return pool;
    }

    if (engine === "mysql") {
      const mysql = await import("mysql2/promise");
      pool = await mysql.createPool(connStr);
      return pool;
    }

    throw new Error(`Database engine "${engine}" not yet supported in JS runtime`);
  };

  const client: DatabaseClient = {
    async query<T = any>(sql: string, params?: any[]): Promise<T[]> {
      const p = await ensurePool();
      if (engine === "postgres") {
        const res = await p.query(sql, params);
        return res.rows as T[];
      }
      if (engine === "mysql") {
        const [rows] = await p.execute(sql, params);
        return rows as T[];
      }
      return [];
    },

    async queryRow<T = any>(sql: string, params?: any[]): Promise<T | null> {
      const rows = await client.query<T>(sql, params);
      return rows[0] ?? null;
    },

    async exec(sql: string, params?: any[]): Promise<{ rowCount: number }> {
      const p = await ensurePool();
      if (engine === "postgres") {
        const res = await p.query(sql, params);
        return { rowCount: res.rowCount ?? 0 };
      }
      if (engine === "mysql") {
        const [result] = await p.execute(sql, params);
        return { rowCount: (result as any).affectedRows ?? 0 };
      }
      return { rowCount: 0 };
    },

    async insert<T = any>(table: string, data: Record<string, any>): Promise<T> {
      const keys = Object.keys(data);
      const values = Object.values(data);
      const placeholders = keys.map((_, i) =>
        engine === "postgres" ? `$${i + 1}` : "?"
      );
      const sql = `INSERT INTO ${table} (${keys.join(", ")}) VALUES (${placeholders.join(", ")}) RETURNING *`;
      const row = await client.queryRow<T>(sql, values);
      return row!;
    },

    get pool() {
      return pool;
    },
  };

  registry.registerDatabase(name, { name, engine, pool: client });

  return client;
}
