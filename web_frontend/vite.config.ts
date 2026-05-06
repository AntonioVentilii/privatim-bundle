import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

const REPLICA_URL = process.env.IC_REPLICA_URL ?? 'http://127.0.0.1:8000';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	define: { global: 'globalThis' },
	optimizeDeps: { esbuildOptions: { target: 'esnext' } },
	build: { target: 'esnext' },
	server: {
		proxy: {
			'/api': {
				target: REPLICA_URL,
				changeOrigin: true
			}
		}
	}
});
