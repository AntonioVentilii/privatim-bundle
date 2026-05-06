// Pure SPA: no SSR, no prerender. The asset canister serves one index.html
// that hydrates client-side and reads runtime config from the `ic_env` cookie.
export const ssr = false;
export const prerender = false;
export const csr = true;
