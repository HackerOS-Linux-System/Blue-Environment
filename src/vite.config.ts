import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

// vite.config.ts is inside src/, so __dirname = <project>/src
// The project root (where index.html lives) is one level up
const projectRoot = resolve(__dirname, '..');

export default defineConfig({
    plugins: [react()],
    // Root is the project root (where index.html lives)
    root: projectRoot,
    clearScreen: false,
    server: {
        port: 1420,
        strictPort: true,
        host: 'localhost',
    },
    envPrefix: ['VITE_', 'TAURI_'],
    build: {
        // Output to <project>/dist — tauri.conf.json distDir is "../dist" relative to src-tauri/
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
