import type { Plugin } from 'vite';
import type { IncomingMessage, ServerResponse } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { GlobalFonts } from '@napi-rs/canvas';

const ROOT       = path.dirname(fileURLToPath(import.meta.url));
const ASSETS_DIR = path.join(ROOT, 'static/img/wf-assets');

// mod-generator's registerFonts() resolves the woff2 path incorrectly —
// pre-register here with the correct absolute path so text renders.
const ROBOTO_WOFF2 = path.join(
  ROOT, 'node_modules/@fontsource-variable/roboto/files/roboto-latin-wght-normal.woff2'
);
if (!GlobalFonts.has('Roboto')) GlobalFonts.registerFromPath(ROBOTO_WOFF2, 'Roboto');

// ── Vite plugin ───────────────────────────────────────────────────────────────

export function modCardPlugin(): Plugin {
  return {
    name: 'mod-card',
    configureServer(server) {
      server.middlewares.use('/api/mod-card', async (req: IncomingMessage, res: ServerResponse) => {
        if (req.method !== 'POST') {
          res.writeHead(405);
          res.end();
          return;
        }

        try {
          const chunks: Buffer[] = [];
          for await (const chunk of req) chunks.push(chunk as Buffer);
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const body: any = JSON.parse(Buffer.concat(chunks).toString());

          const itemType   = body.itemType   ?? '';
          const imageName  = body.imageName  ?? '';
          const name       = body.name       ?? '';
          const rarity     = body.rarity     ?? 'Common';
          const polarity   = body.polarity   ?? 'naramon';
          const maxRank    = Number(body.maxRank ?? 0);
          const rank       = Number(body.rank    ?? 0);
          const full       = body.full === true;
          const compatName  = body.compatName  ?? '';
          const description = body.description ?? '';
          const baseDrain   = Number(body.baseDrain ?? 0);

          // levelStats comes as a JSON string (serialised by Rust) or null
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          let levelStats: any[] | undefined;
          if (body.levelStats) {
            try { levelStats = JSON.parse(body.levelStats); } catch { /* ignore */ }
          }

          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const mod: any = {
            name,
            imageName,
            fusionLimit: levelStats ? levelStats.length - 1 : maxRank,
            rarity,
            polarity,
            baseDrain,
            type: body.type ?? 'Mod',
            compatName,
            description,
            levelStats,
            modSet: undefined,
          };

          let imageArg: string | undefined;
          if (imageName) {
            const avif = imageName.replace(/\.(png|jpg|jpeg|webp)$/i, '.avif');
            const candidate = path.join(ASSETS_DIR, avif);
            try {
              const { existsSync } = await import('node:fs');
              if (existsSync(candidate)) imageArg = candidate;
            } catch { /* skip */ }
          }

          const { default: generate, generateCollapsed } = await import('@wfcd/mod-generator');
          const fn     = full ? generate : generateCollapsed;
          const buffer = await fn({ mod, rank, image: imageArg });

          res.writeHead(200, { 'Content-Type': 'image/png', 'Cache-Control': 'public,max-age=3600' });
          res.end(buffer);
        } catch (err) {
          console.error('[mod-card]', err);
          res.writeHead(500);
          res.end();
        }
      });
    },
  };
}
