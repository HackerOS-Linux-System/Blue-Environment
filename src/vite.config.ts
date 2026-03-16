import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

export default defineConfig({
    plugins: [react()],
                            root: __dirname,
                            clearScreen: false,
                            server: {
                                port: 1420,
                                strictPort: true,
                            },
                            envPrefix: ['VITE_', 'TAURI_'],
                            build: {
                                outDir: resolve(__dirname, '../dist'),
                            emptyOutDir: true,
                            target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
                            minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
                            sourcemap: !!process.env.TAURI_DEBUG,
                            chunkSizeWarningLimit: 1000,
                            rollupOptions: {
                                output: {
                                    manualChunks: {
                                        'react-vendor': ['react', 'react-dom'],
                            'lucide': ['lucide-react'],
                            'genai': ['@google/genai'],
                                    },
                                },
                            },
                            },
});
