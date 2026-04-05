import { api } from "cooper/api";
import { queue } from "cooper/queue";
import { cron } from "cooper/cron";
import { PostPublished } from "../posts/api";

// Notification queue
export const NotifyQueue = queue<{ userId: string; message: string; channel: string }>(
  "notifications",
  {
    concurrency: 10,
    retries: 3,
    retryDelay: "exponential",
    timeout: "10s",
    deadLetter: "notification-dlq",
  }
);

// Dead letter queue for failed notifications
export const NotifyDLQ = queue<{ userId: string; message: string; channel: string }>(
  "notification-dlq"
);

// Subscribe to post published events
export const onPostPublished = PostPublished.subscribe("notify-followers", {
  concurrency: 5,
  handler: async ({ postId, title, authorId }) => {
    console.log(`[notify] Post "${title}" published by ${authorId}`);

    // Queue notifications for followers
    await NotifyQueue.enqueue({
      userId: authorId,
      message: `New post: ${title}`,
      channel: "email",
    });
    await NotifyQueue.enqueue({
      userId: authorId,
      message: `New post: ${title}`,
      channel: "push",
    });
  },
});

// Worker that sends notifications
export const notifyWorker = NotifyQueue.worker("send-notification", {
  handler: async ({ userId, message, channel }) => {
    console.log(`[notify] Sending ${channel} notification to ${userId}: ${message}`);
    // In production: call SendGrid, FCM, Twilio, etc.
  },
  onFailure: async (data, error) => {
    console.error(`[notify] Failed to notify ${data.userId}: ${error.message}`);
  },
});

// Daily digest cron
export const dailyDigest = cron("daily-digest", {
  schedule: "0 9 * * 1-5",
  handler: async () => {
    console.log("[cron] Daily digest would be sent here");
  },
});

// List notifications endpoint
export const listNotifications = api(
  { method: "GET", path: "/notifications", auth: true },
  async ({}, principal) => {
    return {
      notifications: [
        { message: "Welcome to Cooper!", read: false, created_at: new Date().toISOString() },
      ],
    };
  }
);

// Replay dead-letter queue
export const replayDLQ = api(
  { method: "POST", path: "/admin/dlq/replay", auth: true },
  async ({}, { role }) => {
    if (role !== "admin") throw new Error("Admin only");
    const jobs = await NotifyDLQ.list();
    for (const job of jobs) {
      await NotifyQueue.enqueue(job.data);
      await NotifyDLQ.delete(job.id);
    }
    return { replayed: jobs.length };
  }
);
