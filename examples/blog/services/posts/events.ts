import { PostPublished, IndexQueue } from "./api";

// Search indexer worker
export const searchIndexer = IndexQueue.worker("index-post", {
  handler: async ({ postId, content }) => {
    console.log(`[search] Indexing post ${postId}: "${content.slice(0, 50)}..."`);
    // In production: call Elasticsearch, Algolia, or vector store
  },
  onFailure: async (data, error) => {
    console.error(`[search] Failed to index post ${data.postId}: ${error.message}`);
  },
});
