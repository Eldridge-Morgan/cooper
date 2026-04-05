import { registry, type AuthHandlerFn } from "./registry.js";

/**
 * Register an auth handler that verifies tokens and returns a principal.
 *
 * ```ts
 * export const auth = authHandler(async (token) => {
 *   const payload = await verifyJWT(token);
 *   return { userId: payload.sub, role: payload.role };
 * });
 * ```
 */
export function authHandler(handler: AuthHandlerFn) {
  registry.setAuthHandler(handler);
  return handler;
}
