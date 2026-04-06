# AI Primitives

Vector store and LLM gateway built in.

## Vector store

```ts
import { vectorStore } from "cooper-stack/ai";

export const embeddings = vectorStore("product-embeddings", {
  dimensions: 1536,
  similarity: "cosine",
});

await embeddings.upsert("doc-1", vector, { title: "..." });
const results = await embeddings.search(queryVector, { topK: 10 });
```

Uses pgvector locally, Pinecone/Weaviate in production.

## LLM gateway

```ts
import { llmGateway } from "cooper-stack/ai";

export const llm = llmGateway({
  primary: { provider: "openai", model: "gpt-4o" },
  fallback: { provider: "anthropic", model: "claude-sonnet-4-20250514" },
  budget: { dailyLimit: "$50" },
});

const response = await llm.chat([{ role: "user", content: "Hello" }]);
const embedding = await llm.embed("search query");
```
