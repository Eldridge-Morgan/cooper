import { UserCreated } from "./api";

// Send welcome email when a user signs up
export const onUserCreated = UserCreated.subscribe("send-welcome", {
  concurrency: 5,
  handler: async ({ userId, email }) => {
    console.log(`[event] Welcome email would be sent to ${email} (userId: ${userId})`);
  },
});
