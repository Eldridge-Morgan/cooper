export type HydrationStrategy = "load" | "visible" | "idle" | "interaction" | "none";

/**
 * Mark a component as an island — it will be hydrated on the client.
 *
 * ```tsx
 * // islands/LikeButton.island.tsx
 * export default island(function LikeButton({ userId, initialCount }) {
 *   const [count, setCount] = useState(initialCount);
 *   return <button onClick={...}>Like ({count})</button>;
 * });
 * ```
 */
export function island<P = any>(
  component: (props: P) => any
): (props: P & { hydrate?: HydrationStrategy }) => any {
  // Mark the component for the bundler
  const wrapper = (props: P & { hydrate?: HydrationStrategy }) => {
    // Server-side: render the component to HTML
    // Client-side: hydrate based on strategy
    return component(props);
  };

  (wrapper as any)._cooper_island = true;
  (wrapper as any)._cooper_hydrate = "load"; // default strategy

  return wrapper;
}
