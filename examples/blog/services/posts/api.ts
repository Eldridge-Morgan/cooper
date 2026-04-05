import { api } from "cooper/api";
import { CooperError } from "cooper";
import { database } from "cooper/db";
import { cache } from "cooper/cache";
import { topic } from "cooper/pubsub";
import { queue } from "cooper/queue";
import { z } from "zod";

export const db = database("main", {
  migrations: "./migrations",
});

const postCache = cache<any>("posts", { ttl: "5m" });

// Events
export const PostPublished = topic<{ postId: string; title: string; authorId: string }>(
  "post-published",
  { deliveryGuarantee: "at-least-once" }
);

// Queue for background indexing
export const IndexQueue = queue<{ postId: string; content: string }>(
  "search-index",
  { concurrency: 5, retries: 2, retryDelay: "exponential" }
);

// Validation
const CreatePostSchema = z.object({
  title: z.string().min(1).max(200),
  body: z.string().min(1),
  tags: z.array(z.string()).optional(),
});

const UpdatePostSchema = z.object({
  title: z.string().min(1).max(200).optional(),
  body: z.string().min(1).optional(),
  published: z.boolean().optional(),
});

// Routes
export const listPosts = api(
  { method: "GET", path: "/posts" },
  async () => {
    const posts = await db.query(
      "SELECT id, title, published, created_at FROM posts ORDER BY created_at DESC LIMIT 50"
    );
    return { posts };
  }
);

export const getPost = api(
  { method: "GET", path: "/posts/:id" },
  async ({ id }: { id: string }) => {
    const post = await postCache.getOrSet(id, async () => {
      return db.queryRow("SELECT * FROM posts WHERE id = $1", [id]);
    });
    if (!post) throw new CooperError("NOT_FOUND", `Post ${id} not found`);
    return { post };
  }
);

export const createPost = api(
  { method: "POST", path: "/posts", auth: true, validate: CreatePostSchema },
  async (req, principal) => {
    const post = await db.queryRow(
      `INSERT INTO posts (title, body, author_id, tags, published)
       VALUES ($1, $2, $3, $4, false)
       RETURNING *`,
      [req.title, req.body, principal.userId, JSON.stringify(req.tags ?? [])]
    );
    return { post };
  }
);

export const updatePost = api(
  { method: "PATCH", path: "/posts/:id", auth: true, validate: UpdatePostSchema },
  async ({ id, ...updates }, principal) => {
    const existing = await db.queryRow("SELECT * FROM posts WHERE id = $1", [id]);
    if (!existing) throw new CooperError("NOT_FOUND", `Post ${id} not found`);

    // Publish event if transitioning to published
    if (updates.published === true && !existing.published) {
      await PostPublished.publish({
        postId: id,
        title: existing.title,
        authorId: existing.author_id,
      });

      // Queue for search indexing
      await IndexQueue.enqueue({
        postId: id,
        content: `${existing.title} ${existing.body}`,
      });
    }

    const post = await db.queryRow(
      `UPDATE posts SET
        title = COALESCE($1, title),
        body = COALESCE($2, body),
        published = COALESCE($3, published),
        updated_at = NOW()
       WHERE id = $4 RETURNING *`,
      [updates.title, updates.body, updates.published, id]
    );

    await postCache.delete(id);
    return { post };
  }
);

export const deletePost = api(
  { method: "DELETE", path: "/posts/:id", auth: true },
  async ({ id }: { id: string }) => {
    const result = await db.exec("DELETE FROM posts WHERE id = $1", [id]);
    if (result.rowCount === 0) throw new CooperError("NOT_FOUND", `Post ${id} not found`);
    await postCache.delete(id);
    return { deleted: true };
  }
);

// Search (simple ILIKE for now, would use vector store in production)
export const searchPosts = api(
  { method: "GET", path: "/posts/search/:query" },
  async ({ query }: { query: string }) => {
    const posts = await db.query(
      "SELECT id, title, created_at FROM posts WHERE published = true AND (title ILIKE $1 OR body ILIKE $1) LIMIT 20",
      [`%${query}%`]
    );
    return { posts, query };
  }
);
