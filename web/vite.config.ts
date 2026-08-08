import {readFileSync} from 'node:fs'
import {fileURLToPath, URL} from 'node:url'

import {PrimeVueResolver} from '@primevue/auto-import-resolver'
import vue from '@vitejs/plugin-vue'
import Components from 'unplugin-vue-components/vite'
import {defineConfig} from 'vite'

const backend = process.env.APOD_API ?? 'http://localhost:51995'

function version(): string {
    const local = (path: string) => fileURLToPath(new URL(path, import.meta.url))

    try {
        const manifest = readFileSync(local('../Cargo.toml'), 'utf8')
        const workspace = /\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/.exec(manifest)?.[1]
        const found = /^\s*version\s*=\s*"([^"]+)"/m.exec(workspace ?? '')?.[1]
        if (found) return found
    } catch {
    }

    const pkg: unknown = JSON.parse(readFileSync(local('./package.json'), 'utf8'))
    return (pkg as { version?: string }).version ?? '0.0.0'
}

export default defineConfig({
    plugins: [vue(), Components({resolvers: [PrimeVueResolver()]})],
    define: {
        __APP_VERSION__: JSON.stringify(version()),
    },
    resolve: {
        alias: {'@': fileURLToPath(new URL('./src', import.meta.url))},
    },
    server: {
        // /pic is where the games fetch their thumbnails, deliberately outside /api so a grid of
        // them is not rate limited. It has to be proxied like the rest or the games have no images.
        proxy: {
            '/api': backend,
            '/thumbs': backend,
            '/pic': backend,
        },
    },
})
