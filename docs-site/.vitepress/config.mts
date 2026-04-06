import { defineConfig } from "vitepress";

export default defineConfig({
  title: "Cooper",
  description: "The backend framework for TypeScript — powered by Rust",
  ignoreDeadLinks: true,
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: "/logo.svg" }],
    ["meta", { name: "theme-color", content: "#000000" }],
  ],
  themeConfig: {
    siteTitle: "cooper",
    nav: [
      { text: "Guide", link: "/guide/getting-started" },
      { text: "Reference", link: "/reference/api" },
      { text: "Deploy", link: "/deploy/overview" },
      { text: "GitHub", link: "https://github.com/Eldridge-Morgan/cooper" },
    ],
    sidebar: {
      "/guide/": [
        {
          text: "Introduction",
          items: [
            { text: "What is Cooper?", link: "/guide/what-is-cooper" },
            { text: "Getting Started", link: "/guide/getting-started" },
            { text: "Project Structure", link: "/guide/project-structure" },
          ],
        },
        {
          text: "Core",
          items: [
            { text: "API Routes", link: "/guide/api-routes" },
            { text: "Validation", link: "/guide/validation" },
            { text: "Middleware", link: "/guide/middleware" },
            { text: "Auth", link: "/guide/auth" },
            { text: "Errors", link: "/guide/errors" },
          ],
        },
        {
          text: "Data",
          items: [
            { text: "Database", link: "/guide/database" },
            { text: "Cache", link: "/guide/cache" },
            { text: "Pub/Sub", link: "/guide/pubsub" },
            { text: "Queues", link: "/guide/queues" },
            { text: "Cron Jobs", link: "/guide/cron" },
          ],
        },
        {
          text: "Advanced",
          items: [
            { text: "Streaming", link: "/guide/streaming" },
            { text: "Storage", link: "/guide/storage" },
            { text: "Secrets", link: "/guide/secrets" },
            { text: "AI Primitives", link: "/guide/ai" },
            { text: "Monorepo", link: "/guide/monorepo" },
          ],
        },
        {
          text: "Frontend",
          items: [
            { text: "Generated Clients", link: "/guide/clients" },
            { text: "SSR & Pages", link: "/guide/ssr" },
            { text: "Islands", link: "/guide/islands" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "API Reference",
          items: [
            { text: "cooper/api", link: "/reference/api" },
            { text: "cooper/db", link: "/reference/db" },
            { text: "cooper/cache", link: "/reference/cache" },
            { text: "cooper/pubsub", link: "/reference/pubsub" },
            { text: "cooper/queue", link: "/reference/queue" },
            { text: "cooper/auth", link: "/reference/auth" },
            { text: "cooper/middleware", link: "/reference/middleware" },
            { text: "cooper/cron", link: "/reference/cron" },
            { text: "cooper/storage", link: "/reference/storage" },
            { text: "cooper/secrets", link: "/reference/secrets" },
            { text: "cooper/ssr", link: "/reference/ssr" },
            { text: "cooper/islands", link: "/reference/islands" },
            { text: "cooper/ai", link: "/reference/ai" },
          ],
        },
        {
          text: "CLI",
          items: [
            { text: "CLI Reference", link: "/reference/cli" },
            { text: "cooper.config.ts", link: "/reference/config" },
          ],
        },
      ],
      "/deploy/": [
        {
          text: "Deployment",
          items: [
            { text: "Overview", link: "/deploy/overview" },
            { text: "AWS", link: "/deploy/aws" },
            { text: "GCP", link: "/deploy/gcp" },
            { text: "Azure", link: "/deploy/azure" },
            { text: "Fly.io", link: "/deploy/fly" },
            { text: "Preview Environments", link: "/deploy/preview" },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: "github", link: "https://github.com/Eldridge-Morgan/cooper" },
    ],
    footer: {
      message: "Apache-2.0 Licensed",
      copyright: "Cooper — The backend framework for TypeScript",
    },
    search: {
      provider: "local",
    },
  },
});
