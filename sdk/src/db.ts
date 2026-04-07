import { registry } from "./registry.js";

export interface DatabaseConfig {
  engine?: "postgres" | "mysql" | "mongodb" | "dynamodb";
  migrations?: string;
  partitionKey?: string;
  sortKey?: string;
}

export interface TransactionClient {
  /** Run a query returning multiple rows */
  query<T = any>(sql: string, params?: any[]): Promise<T[]>;
  /** Run a query returning a single row or null */
  queryRow<T = any>(sql: string, params?: any[]): Promise<T | null>;
  /** Run a query returning affected row count */
  exec(sql: string, params?: any[]): Promise<{ rowCount: number }>;
  /** Insert and return the inserted row */
  insert<T = any>(table: string, data: Record<string, any>): Promise<T>;
  /** Access the underlying connection (single client, not pool) */
  conn: any;
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
  /** Run a callback inside a transaction — auto-commits on success, rolls back on error */
  transaction<R = any>(fn: (tx: TransactionClient) => Promise<R>): Promise<R>;
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
    ?? `postgres://cooper@localhost:5432/cooper_${name}`;

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

    async transaction<R = any>(fn: (tx: TransactionClient) => Promise<R>): Promise<R> {
      const p = await ensurePool();

      if (engine === "postgres") {
        const pgClient = await p.connect();
        try {
          await pgClient.query("BEGIN");

          const tx: TransactionClient = {
            async query<T = any>(sql: string, params?: any[]): Promise<T[]> {
              const res = await pgClient.query(sql, params);
              return res.rows as T[];
            },
            async queryRow<T = any>(sql: string, params?: any[]): Promise<T | null> {
              const res = await pgClient.query(sql, params);
              return (res.rows[0] as T) ?? null;
            },
            async exec(sql: string, params?: any[]): Promise<{ rowCount: number }> {
              const res = await pgClient.query(sql, params);
              return { rowCount: res.rowCount ?? 0 };
            },
            async insert<T = any>(table: string, data: Record<string, any>): Promise<T> {
              const keys = Object.keys(data);
              const values = Object.values(data);
              const placeholders = keys.map((_, i) => `$${i + 1}`);
              const sql = `INSERT INTO ${table} (${keys.join(", ")}) VALUES (${placeholders.join(", ")}) RETURNING *`;
              const res = await pgClient.query(sql, values);
              return res.rows[0] as T;
            },
            get conn() { return pgClient; },
          };

          const result = await fn(tx);
          await pgClient.query("COMMIT");
          return result;
        } catch (err) {
          await pgClient.query("ROLLBACK");
          throw err;
        } finally {
          pgClient.release();
        }
      }

      if (engine === "mysql") {
        const conn = await p.getConnection();
        try {
          await conn.beginTransaction();

          const tx: TransactionClient = {
            async query<T = any>(sql: string, params?: any[]): Promise<T[]> {
              const [rows] = await conn.execute(sql, params);
              return rows as T[];
            },
            async queryRow<T = any>(sql: string, params?: any[]): Promise<T | null> {
              const [rows] = await conn.execute(sql, params);
              return ((rows as any[])[0] as T) ?? null;
            },
            async exec(sql: string, params?: any[]): Promise<{ rowCount: number }> {
              const [result] = await conn.execute(sql, params);
              return { rowCount: (result as any).affectedRows ?? 0 };
            },
            async insert<T = any>(table: string, data: Record<string, any>): Promise<T> {
              const keys = Object.keys(data);
              const values = Object.values(data);
              const placeholders = keys.map(() => "?");
              const sql = `INSERT INTO ${table} (${keys.join(", ")}) VALUES (${placeholders.join(", ")})`;
              await conn.execute(sql, values);
              const [rows] = await conn.execute(`SELECT * FROM ${table} WHERE id = LAST_INSERT_ID()`);
              return (rows as any[])[0] as T;
            },
            get conn() { return conn; },
          };

          const result = await fn(tx);
          await conn.commit();
          return result;
        } catch (err) {
          await conn.rollback();
          throw err;
        } finally {
          conn.release();
        }
      }

      throw new Error(`Transactions not supported for engine "${engine}"`);
    },

    get pool() {
      return pool;
    },
  };

  registry.registerDatabase(name, { name, engine, pool: client });

  return client;
}
