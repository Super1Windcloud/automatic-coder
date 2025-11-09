# Repository Guidelines

## Project Structure & Module Organization
The React/Vite client lives in `src/`, split into `components/` (reusable UI), `pages/` (route-level views), `services/` (API and device bridges), `store/` (Zustand state), and `utils/`. Activation-specific helpers reside in `activation.ts` and `select.ts`. Static assets that ship with the web bundle stay in `public/`, while generated builds land in `dist/` (web) and `bundle/` (signed Tauri installers). Native code, license tooling, and activation assets live in `src-tauri/`; `src-tauri/src/` holds the Tauri commands, `license_manager/` builds the activation generator, and `assets/` stores encrypted activation payloads.

## Build, Test, and Development Commands
Use `pnpm dev` for the Vite frontend only, or `pnpm td` to boot the full Tauri shell (Rust backend + webview). `pnpm build` performs type-checking via `tsc` before running the default Vite build; `pnpm custom` aggregates the two branded builds (`pnpm build:select` and `pnpm build:activation`). Package desktop installers with `pnpm tb`, which runs `tauri build`, cleans stale bundles, and copies artifacts into `bundle/`. Quality gates: `pnpm lint` (ESLint + `cargo check`) and `pnpm format` (Prettier + `cargo fmt`). Clean Rust artifacts with `pnpm clean`, and use `pnpm release` for a CLI-only `cargo build --release`.

## Coding Style & Naming Conventions
TypeScript/TSX are preferred everywhere; keep files ES modules. Follow Prettier defaults (2-space indentation, single quotes) and rely on Biome via `pnpm fix` for mechanical refactors. Components, hooks, and Zustand stores are PascalCase files (`InterviewPanel.tsx`), functions/constants camelCase, and environment variables SCREAMING_SNAKE_CASE. When wiring UI to native commands, colocate bridge helpers under `services/tauri/` and export typed wrappers so React code stays declarative.

## Testing Guidelines
There is no dedicated frontend test runner yet; guard regressions with `pnpm lint`, targeted storybook-style screenshots, and manual activation flows. Add future unit tests under `src/__tests__/` using Vitest (mirroring `*.test.tsx`) and end-to-end checks under `tests/e2e/` once Playwright is introduced. Rust modules should include `#[cfg(test)]` blocks inside the relevant files and run via `cargo test` from `src-tauri/`. Aim for coverage on new commands, especially anything touching activation persistence or filesystem IO.

## Commit & Pull Request Guidelines
Recent history favors single-word imperative subjects such as `update`; keep that tone but add context (`feat(editor): add audio prompts`) to improve traceability. Each PR should include: a short summary of motivation, screenshots or terminal output for UI or activation changes, linked issue or task, and a checklist confirming `pnpm lint`, `pnpm td`, and the relevant cargo commands passed. Coordinate release branches with the `just bundle` recipe, and avoid force-pushing to `master` unless the maintainer signs off.

## Security & Activation Essentials
Never commit real keys—`.env` files under both roots are gitignored; use `.env.example` updates for config changes. Before distributing a build, regenerate activation artifacts with `cargo run -p license_manager --bin generate -- assets "<ACTIVATION_MASTER_KEY>" 10000 16` and ensure `assets/activation_codes.enc` ships in the installer bundle. Treat `bundle/` and `src-tauri/target/` as ephemeral; purge them after testing to reduce risk of leaking signed binaries.
