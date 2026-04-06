# Database

Declare databases in code. Cooper starts embedded Postgres locally and provisions managed databases on deploy.

## Declare

```ts
import { database } from "cooper-stack/db";

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

## Transactions

Wrap multiple queries in a transaction. Auto-commits on success, rolls back on error.

```ts
const order = await db.transaction(async (tx) => {
  const order = await tx.insert("orders", { user_id: userId, total: 99.99 });
  await tx.exec("UPDATE inventory SET stock = stock - 1 WHERE product_id = $1", [productId]);
  await tx.exec("INSERT INTO order_items (order_id, product_id) VALUES ($1, $2)", [order.id, productId]);
  return order;
});
```

The `tx` object has the same API as `db`:

| Method | Description |
|---|---|
| `tx.query<T>(sql, params?)` | Multiple rows |
| `tx.queryRow<T>(sql, params?)` | Single row or null |
| `tx.exec(sql, params?)` | Execute, return rowCount |
| `tx.insert<T>(table, data)` | Insert and return row |
| `tx.conn` | Underlying connection (for ORMs) |

If the callback throws, the transaction rolls back and the error propagates:

```ts
try {
  await db.transaction(async (tx) => {
    await tx.exec("UPDATE accounts SET balance = balance - 500 WHERE id = $1", [fromId]);
    await tx.exec("UPDATE accounts SET balance = balance + 500 WHERE id = $1", [toId]);

    const from = await tx.queryRow("SELECT balance FROM accounts WHERE id = $1", [fromId]);
    if (from.balance < 0) throw new Error("Insufficient funds");
  });
} catch (err) {
  // Transaction was rolled back — both accounts unchanged
}
```

### Transactions with Drizzle

Use `tx.conn` to get the underlying client for ORM operations inside a transaction:

```ts
await db.transaction(async (tx) => {
  const orm = drizzle(tx.conn);
  await orm.insert(orders).values({ userId, total: 99.99 });
  await orm.update(inventory).set({ stock: sql`stock - 1` }).where(eq(inventory.productId, productId));
});
```

Works with both PostgreSQL and MySQL.

## Engines

| Engine | Local | AWS | GCP | Azure | Fly |
|---|---|---|---|---|---|
| `postgres` | Embedded | RDS | Cloud SQL | Azure DB | Fly Postgres |
| `mysql` | Embedded | RDS | Cloud SQL | Azure DB | — |
