import { authHandler } from "cooper/auth";
import { api } from "cooper/api";
import { CooperError } from "cooper";

// Register the auth handler — called for every route with auth: true
export const auth = authHandler(async (token: string) => {
  // Replace with your actual JWT verification
  // For now, accept any non-empty token
  if (!token || token === "invalid") {
    throw new CooperError("UNAUTHORIZED", "Invalid token");
  }

  // Return the principal — injected into protected route handlers
  return {
    userId: "user_from_token",
    role: "user",
  };
});
