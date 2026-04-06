# cooper.config.ts

```ts
import { secret } from "cooper-stack/secrets";

export default {
  name: "my-app",

  ssr: {
    framework: "react",       // "react" | "solid" | "vue"
    assets: {
      cdn: true,
      compress: true,
      imageOptimization: true,
    },
    fonts: {
      selfHost: true,
    },
  },

  observability: {
    traces:  { provider: "datadog",  apiKey: secret("dd-key") },
    metrics: { provider: "grafana",  endpoint: secret("grafana-url") },
    logs:    { provider: "axiom",    token: secret("axiom-token") },
    // or raw OpenTelemetry
    otel:    { endpoint: "http://otel-collector:4317" },
  },

  docs: {
    title: "My API",
    description: "Internal platform API",
    contact: "platform@myapp.com",
  },
};
```
