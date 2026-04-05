import { secret } from "cooper/secrets";

export default {
  name: "cooper-example",
  ssr: {
    framework: "react",
    assets: {
      cdn: true,
      compress: true,
    },
  },
  observability: {
    // traces: { provider: "datadog", apiKey: secret("dd-key") },
    // metrics: { provider: "grafana", endpoint: secret("grafana-url") },
  },
  docs: {
    title: "cooper-example API",
    description: "API documentation",
  },
};
