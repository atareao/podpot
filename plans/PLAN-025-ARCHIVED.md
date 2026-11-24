# Plan 025 – Reduce Ant Design Bundle Size (Raw) Below 500 kB

## Purpose

This plan addresses issue #56 and the OpenSpec change `chunk-antd`. The frontend's Ant Design vendor chunks exceed the 500 kB raw size threshold, impacting load performance. This plan documents the steps taken to reduce the bundle size while maintaining full test coverage and no breaking changes.

- **Issue:** #56
- **OpenSpec change:** `chunk-antd`

## Scope

**Frontend only.** Changes are limited to:

| File | Role |
|------|------|
| `frontend/vite.config.ts` | Terser minification, chunk splitting tuning |
| `frontend/src/components/FormularioDeCreacion.tsx` | Lazy-loaded component |
| `frontend/src/components/TablaDeUnidades.tsx` | Lazy-loaded component |
| `frontend/tests/tamanoDelBundle.test.ts` | Bundle-size regression test |
| Terser minification | Configuration adjustments |

No backend changes. No API contract changes.

## Goals

1. **Reduce raw size** of `vendor-antd` and `vendor-antd-rc` chunks to **≤ 500 kB** each.
2. **Keep all UI tests passing** — 141 tests total, zero regressions.
3. **No breaking changes** — public API, component props, and routing remain unchanged.

## Tasks

### 1. Analyze current bundle output

- [ ] Run `pnpm --dir frontend build` and capture chunk sizes.
- [ ] Identify which Ant Design imports contribute the most to `vendor-antd`.
- [ ] Check whether barrel imports (`import { Button } from 'antd'`) pull in unnecessary modules.

### 2. Enable Terser minification

- [ ] Add `terser` as a `devDependency` in `frontend/package.json`.
- [ ] Configure `build.minify: 'terser'` in `vite.config.ts`.
- [ ] Tune `turbosquash`/`drop_console` options to strip dev-only code.

### 3. Split vendor chunks

- [ ] Configure `rollupOptions.output.manualChunks` to separate `antd` and `antd-icons`/`rc-*` into distinct vendor chunks.
- [ ] Verify `vendor-antd-rc` is a separate chunk from `vendor-antd`.

### 4. Lazy-load heavy components

- [ ] Convert `FormularioDeCreacion.tsx` to `React.lazy` + `Suspense` so it loads on demand.
- [ ] Convert `TablaDeUnidades.tsx` similarly.
- [ ] Ensure fallback/loading states are accessible.

### 5. Update bundle-size regression test

- [ ] Add `frontend/tests/tamanoDelBundle.test.ts` that asserts each vendor chunk is ≤ 500 kB raw.
- [ ] Wire the test into the existing Vitest suite.

### 6. Verify

- [ ] Run `pnpm --dir frontend test` — all 141 tests pass.
- [ ] Run `pnpm --dir frontend build` — confirm both `vendor-antd` and `vendor-antd-rc` are ≤ 500 kB.
- [ ] Manually verify the UI still renders correctly (lazy-loaded components show properly).

## Deliverables

- **Updated files:** `vite.config.ts`, `FormularioDeCreacion.tsx`, `TablaDeUnidades.tsx`, `tamanoDelBundle.test.ts`, `package.json`
- **Git diff stat:** summary of lines changed per file.
- **Test results:** full Vitest output showing 141/141 passing.
- **Bundle size report:** before/after chunk sizes in kB.
- **Documentation:** `docs/bundle-size.md` describing the chunk strategy and Terser config.

## Status

Archived — all tasks completed and verified.