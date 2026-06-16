import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';
import tailwindcss from 'tailwindcss';
import autoprefixer from 'autoprefixer';

const projectRoot = resolve(__dirname, '..');

export default defineConfig({
    plugins: [react()],
                            root: projectRoot,
                            clearScreen: false,
                            css: {
                                postcss: {
                                    plugins: [
                                        tailwindcss({ config: resolve(projectRoot, 'tailwind.config.js') }),
                            autoprefixer(),
                                    ],
                                },
                            },
                            server: {
                                port: 1420,
                            strictPort: true,
                            host: 'localhost',
                            },
                            envPrefix: ['VITE_', 'TAURI_'],
                            build: {
                                outDir: resolve(projectRoot, 'dist'),
                            emptyOutDir: true,
                            target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
                            minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
                            sourcemap: !!process.env.TAURI_DEBUG,
                            chunkSizeWarningLimit: 1500,
                            rollupOptions: {
                                input: resolve(projectRoot, 'index.html'),
                            output: {
                                manualChunks: {
                                    'react-vendor': ['react', 'react-dom'],
                            'lucide': ['lucide-react'],
                                },
                            },
                            },
                            },
});
