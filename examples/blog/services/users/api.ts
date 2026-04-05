import { api } from "cooper/api";
import { CooperError } from "cooper";
import { database } from "cooper/db";
import { cache } from "cooper/cache";
import { topic } from "cooper/pubsub";
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
