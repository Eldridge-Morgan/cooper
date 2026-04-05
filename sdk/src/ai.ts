export interface VectorStoreConfig {
  dimensions: number;
  similarity?: "cosine" | "euclidean" | "dot";
}

export interface VectorStoreClient {
  upsert(id: string, vector: number[], metadata?: Record<string, any>): Promise<void>;
  search(vector: number[], opts?: { topK?: number; filter?: Record<string, any> }): Promise<{ id: string; score: number; metadata?: any }[]>;
  delete(id: string): Promise<void>;
}

export interface LLMGatewayConfig {
  primary: { provider: string; model: string };
  fallback?: { provider: string; model: string };
  budget?: { dailyLimit: string };
}

export interface LLMGatewayClient {
  chat(messages: { role: string; content: string }[]): Promise<string>;
  embed(text: string): Promise<number[]>;
}

/**
 * Declare a vector store.
 *
 * ```ts
 * export const embeddings = vectorStore("product-embeddings", {
 *   dimensions: 1536,
 *   similarity: "cosine",
 * });
 * ```
 */
export function vectorStore(name: string, config: VectorStoreConfig): VectorStoreClient {
  // Local dev: pgvector extension on embedded Postgres
  // Production: Pinecone, Weaviate, or pgvector on managed Postgres
  const store = new Map<string, { vector: number[]; metadata?: any }>();

  const cosineSimilarity = (a: number[], b: number[]): number => {
    let dot = 0, magA = 0, magB = 0;
    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      magA += a[i] * a[i];
      magB += b[i] * b[i];
    }
    return dot / (Math.sqrt(magA) * Math.sqrt(magB));
  };

  return {
    async upsert(id, vector, metadata) {
      store.set(id, { vector, metadata });
    },

    async search(vector, opts) {
      const topK = opts?.topK ?? 10;
      const results: { id: string; score: number; metadata?: any }[] = [];

      for (const [id, entry] of store) {
        const score = cosineSimilarity(vector, entry.vector);
        results.push({ id, score, metadata: entry.metadata });
      }

      results.sort((a, b) => b.score - a.score);
      return results.slice(0, topK);
    },

    async delete(id) {
      store.delete(id);
    },
  };
}

/**
 * Declare an LLM gateway with cost tracking and fallbacks.
 *
 * ```ts
 * export const llm = llmGateway({
 *   primary: { provider: "openai", model: "gpt-4o" },
 *   fallback: { provider: "anthropic", model: "claude-sonnet-4-20250514" },
 *   budget: { dailyLimit: "$50" },
 * });
 * ```
 */
export function llmGateway(config: LLMGatewayConfig): LLMGatewayClient {
  return {
    async chat(messages) {
      // Route to the configured provider
      // Track cost, apply rate limits, handle fallback
      throw new Error(
        `LLM gateway not yet connected. Configure API keys via: cooper secrets set ${config.primary.provider}-api-key --env local`
      );
    },

    async embed(text) {
      throw new Error(
        `LLM gateway not yet connected. Configure API keys via: cooper secrets set ${config.primary.provider}-api-key --env local`
      );
    },
  };
}
