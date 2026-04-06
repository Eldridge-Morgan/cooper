/**
 * Declare a secret — fetched at runtime, never in .env or source code.
 *
 * ```ts
 * const stripeKey = secret("stripe-api-key");
 * const client = new Stripe(await stripeKey());
 * ```
 */
export function secret(name: string): () => Promise<string> {
  let cached: string | null = null;

  const resolve = async (): Promise<string> => {
    if (cached !== null) return cached;

    // 1. Check env var (set by `cooper secrets set`)
    const envKey = `COOPER_SECRET_${name.toUpperCase().replace(/-/g, "_")}`;
    const envVal = process.env[envKey];
    if (envVal) {
      cached = envVal;
      return cached;
    }

    // 2. Check .cooper/secrets/<env>/<name>
    const fs = require("node:fs");
    const path = require("node:path");
    const env = process.env.COOPER_ENV ?? "local";
    const secretPath = path.join(".cooper", "secrets", env, name);
    if (fs.existsSync(secretPath)) {
      cached = fs.readFileSync(secretPath, "utf-8").trim();
      return cached;
    }

    throw new Error(
      `Secret "${name}" not found. Set it with: cooper secrets set ${name} --env ${env}`
    );
  };

  return resolve;
}
