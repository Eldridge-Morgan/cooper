import { registry, type MiddlewareFn } from "./registry.js";

/**
 * Define a middleware function.
 *
 * ```ts
 * const rateLimiter = middleware(async (req, next) => {
 *   const count = await userCache.increment(`rate:${req.ip}`, { ttl: "1m" });
 *   if (count > 100) throw new CooperError("RATE_LIMITED");
 *   return next(req);
 * });
 * ```
 */
export function middleware(fn: MiddlewareFn): MiddlewareFn {
  return fn;
}

/**
 * Cooper instance for global middleware registration.
 */
export const cooper = {
  use(...middlewares: MiddlewareFn[]) {
    for (const mw of middlewares) {
      registry.addGlobalMiddleware(mw);
    }
  },
};
