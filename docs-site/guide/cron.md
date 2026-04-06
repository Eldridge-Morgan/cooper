# Cron Jobs

Scheduled tasks with human-readable or standard cron syntax.

```ts
import { cron } from "cooper/cron";

export const cleanup = cron("cleanup", {
  schedule: "every 1 hour",
  handler: async () => {
    await db.query("DELETE FROM sessions WHERE expires_at < NOW()");
  },
});

export const dailyReport = cron("daily-report", {
  schedule: "0 9 * * 1-5",  // 9am Mon-Fri
  handler: async () => {
    const report = await generateReport();
    await emailTeam(report);
  },
});
```

## Schedule formats

| Format | Example |
|---|---|
| Human-readable | `"every 30 minutes"`, `"every 1 hour"`, `"every 1 day"` |
| Cron expression | `"0 9 * * 1-5"`, `"*/15 * * * *"` |
