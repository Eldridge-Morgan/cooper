import { cron } from "cooper/cron";

export const sessionCleanup = cron("session-cleanup", {
  schedule: "every 1 hour",
  handler: async () => {
    console.log("[cron] Session cleanup would run here");
  },
});
