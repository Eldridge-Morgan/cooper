import { registry } from "./registry.js";

export interface CronConfig {
  schedule: string;
  handler: () => Promise<void>;
}

/**
 * Declare a cron job.
 *
 * ```ts
 * export const cleanup = cron("cleanup", {
 *   schedule: "every 1 hour",
 *   handler: async () => {
 *     await db.query("DELETE FROM sessions WHERE expires_at < NOW()");
 *   },
 * });
 * ```
 */
export function cron(name: string, config: CronConfig) {
  registry.registerCron(name, {
    name,
    schedule: config.schedule,
    handler: config.handler,
  });

  return {
    _cooper_type: "cron" as const,
    name,
    schedule: config.schedule,
  };
}
