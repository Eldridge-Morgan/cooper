import { api } from "cooper/api";

export const healthCheck = api(
  { method: "GET", path: "/health" },
  async () => {
    return {
      status: "ok",
      timestamp: new Date().toISOString(),
      uptime: process.uptime(),
    };
  }
);
