export interface PageConfig {
  cache?: string;
  auth?: boolean;
}

export interface PageContext {
  params: Record<string, string>;
  req?: any;
  data?: any;
}

/**
 * Define a server-rendered page.
 *
 * ```ts
 * export default page(async ({ params }) => {
 *   const { user } = await UsersService.getUser({ id: params.id });
 *   return <div><h1>{user.name}</h1></div>;
 * });
 * ```
 */
export function page<T = any>(
  render: (ctx: PageContext) => Promise<T>
): { _cooper_type: "page"; render: typeof render } {
  return { _cooper_type: "page", render };
}

/**
 * Define a layout that wraps pages.
 *
 * ```ts
 * export default layout(({ children }) => (
 *   <div><Nav /><main>{children}</main><Footer /></div>
 * ));
 * ```
 */
export function layout<T = any>(
  render: (props: { children: any }) => T
): { _cooper_type: "layout"; render: typeof render } {
  return { _cooper_type: "layout", render };
}

/**
 * Define a page data loader — runs on the server, data passed to the page.
 */
export function pageLoader<T>(
  loader: (ctx: PageContext) => Promise<T>
): { _cooper_type: "loader"; loader: typeof loader } {
  return { _cooper_type: "loader", loader };
}

/**
 * Suspense boundary for streaming SSR.
 */
export function Suspense(_props: { fallback: any; children: any }) {
  // Handled by the SSR renderer — this is a marker component
  return null;
}
