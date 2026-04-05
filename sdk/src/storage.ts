export interface BucketConfig {
  public?: boolean;
}

export interface BucketClient {
  upload(key: string, data: Buffer | Uint8Array, opts?: { contentType?: string }): Promise<void>;
  download(key: string): Promise<Buffer>;
  signedUrl(key: string, opts?: { ttl?: string }): Promise<string>;
  delete(key: string): Promise<void>;
  list(prefix?: string): Promise<{ key: string; size: number; lastModified: Date }[]>;
}

/**
 * Declare an object storage bucket.
 *
 * ```ts
 * export const avatars = bucket("avatars", { public: true });
 * await avatars.upload("user-123/avatar.png", buffer, { contentType: "image/png" });
 * ```
 */
export function bucket(name: string, config?: BucketConfig): BucketClient {
  const fs = require("node:fs");
  const path = require("node:path");

  // Local storage directory — in production this maps to S3/GCS/Azure Blob
  const localDir = path.join(
    process.env.COOPER_STORAGE_DIR ?? ".cooper/storage",
    name
  );

  return {
    async upload(key: string, data: Buffer | Uint8Array, opts?: { contentType?: string }) {
      const fullPath = path.join(localDir, key);
      fs.mkdirSync(path.dirname(fullPath), { recursive: true });
      fs.writeFileSync(fullPath, data);
    },

    async download(key: string): Promise<Buffer> {
      const fullPath = path.join(localDir, key);
      return fs.readFileSync(fullPath);
    },

    async signedUrl(key: string, opts?: { ttl?: string }): Promise<string> {
      // In local dev, return a direct file path
      // In production, generate a presigned S3/GCS URL
      return `http://localhost:9400/storage/${name}/${key}`;
    },

    async delete(key: string): Promise<void> {
      const fullPath = path.join(localDir, key);
      if (fs.existsSync(fullPath)) fs.unlinkSync(fullPath);
    },

    async list(prefix?: string) {
      const dir = prefix ? path.join(localDir, prefix) : localDir;
      if (!fs.existsSync(dir)) return [];

      const entries: { key: string; size: number; lastModified: Date }[] = [];
      const walk = (d: string, rel: string) => {
        for (const entry of fs.readdirSync(d, { withFileTypes: true })) {
          const fullPath = path.join(d, entry.name);
          const relPath = rel ? `${rel}/${entry.name}` : entry.name;
          if (entry.isDirectory()) {
            walk(fullPath, relPath);
          } else {
            const stat = fs.statSync(fullPath);
            entries.push({
              key: relPath,
              size: stat.size,
              lastModified: stat.mtime,
            });
          }
        }
      };
      walk(dir, "");
      return entries;
    },
  };
}
