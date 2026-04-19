# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Interview-Coder is a Tauri-based desktop application that combines screen capture, AI-driven coding assistance, and a secure license activation system. The frontend is React/Vite with TypeScript, and the backend is Rust with Tauri v2.

## Architecture

### Frontend Structure (`src/`)
- `components/` - Reusable UI components
- `pages/` - Route-level views
- `services/` - API clients and Tauri command bridges
- `store/` - Zustand state management
- `lib/` - Shared utilities
- Entry points: `activation.ts`, `select.ts`, `openai.ts`, `host.ts` for different build targets

### Backend Structure (`src-tauri/`)
- `src/lib.rs` - Main library entry point
- `src/license.rs` - License validation and activation logic
- `src/vlm.rs` - Vision Language Model integration
- `src/capture.rs` - Screen capture functionality
- `src/system.rs` - System utilities and fingerprinting
- `src/lan.rs` - Network/LAN features
- `src/config.rs` - Configuration management
- `license_manager/` - Standalone license generation tools

### License System Architecture

The activation system uses a three-tier verification:
1. Encrypted activation codes stored remotely on GitHub Releases
2. Machine fingerprinting stored in three locations (Documents, Local AppData, Roaming AppData)
3. One-time code consumption with remote state sync

Activation codes are generated with `license_manager`, encrypted, and distributed. Upon activation, the code is consumed from the remote store and a fingerprint is written locally.

## Development Commands

### Frontend Development
```bash
pnpm dev              # Vite dev server only
pnpm td               # Full Tauri dev (Rust + webview)
pnpm build            # TypeScript check + Vite build
pnpm custom           # Build all variants (select, activation, openai, host)
```

### Backend Development
```bash
cd src-tauri && cargo build          # Debug build
pnpm release                          # Release build
pnpm clean                            # Clean Rust artifacts
cd src-tauri && cargo test            # Run Rust tests
```

### Quality & Formatting
```bash
pnpm lint             # ESLint + cargo check
pnpm format           # Prettier + cargo fmt
pnpm fix              # Biome auto-fix
```

### Building & Bundling
```bash
pnpm tb               # Tauri build + bundle cleanup
just bundle           # Same as pnpm tb (via justfile)
```

### Publishing
```bash
pnpm publish:new                      # Auto-bump version + build + upload
pnpm publish:new:windows              # Windows-specific release
pnpm publish:new:macos                # macOS-specific release
just dmg                              # Build + publish macOS
just nsis                             # Build + publish Windows
```

### License Management
```bash
# Generate activation codes (10,000 codes with 16-char length)
just license-generate-keys

# Issue a license
just license-issue <private_key> <machine_id> <license_id> [expires_days] [customer]

# Sign revocations
just license-sign-revocations <private_key> <input_json> [output_json]

# Get machine ID
just license-machine-id
```

## Key Files

- `tauri.conf.json` - Tauri configuration, window settings, updater config
- `justfile` - Task runner with common commands (preferred over npm scripts for complex tasks)
- `package.json` - Frontend dependencies and npm scripts
- `Cargo.toml` - Rust dependencies and workspace configuration
- `.env` - Environment variables (gitignored, contains secrets)
- `AGENTS.md` - Existing repository guidelines (complementary to this file)

## Environment Variables

Required in `src-tauri/.env`:
```
ACTIVATION_MASTER_KEY=<your_key>
GITHUB_TOKEN=<your_github_pat>
GITHUB_OWNER=Super1Windcloud
GITHUB_REPO=automatic-coder
GITHUB_RELEASE_TAG=v1.0.0
```

Also in root `.env` for frontend builds.

## Coding Conventions

- TypeScript/TSX for all frontend code
- PascalCase for components/hooks (`InterviewPanel.tsx`)
- camelCase for functions/constants
- SCREAMING_SNAKE_CASE for environment variables
- Prettier: 2-space indentation, single quotes
- Tauri commands: bridge via typed wrappers in `services/tauri/`

## Testing

- Frontend: No test runner yet; use `pnpm lint` and manual testing
- Backend: `#[cfg(test)]` blocks in Rust files, run with `cargo test`
- Focus testing on activation logic and filesystem operations

## Build Artifacts

- `dist/` - Vite build output (web bundle)
- `bundle/` - Signed Tauri installers (ephemeral, clean after testing)
- `src-tauri/target/` - Rust build artifacts (ephemeral)
- `src-tauri/assets/activation_codes.enc` - Encrypted activation store (ships with installer)

## Security Notes

- Never commit `.env` files or real activation keys
- `bundle/` and `src-tauri/target/` are ephemeral; purge after testing
- Regenerate activation codes before distribution
- Activation fingerprints must exist in all three locations to be valid

## Tauri Commands

Tauri commands are defined in `src-tauri/src/` files and exposed via `#[tauri::command]`. Frontend calls them via `@tauri-apps/api/core::invoke()`. Bridge wrappers should be in `src/services/tauri/`.

## Multi-Window Architecture

The app uses multiple HTML entry points for different contexts:
- `index.html` - Main application
- `activation.html` - Activation window
- `select.html` - Selection UI
- `openai.html` - OpenAI integration
- `host.html` - Host management

Each has a corresponding Vite config (`vite.*.config.ts`).

## Updater

Auto-updates via Tauri plugin, configured in `tauri.conf.json`. Fetches `latest.json` from GitHub Releases. Update scripts in `scripts/` handle version bumping and asset uploads.

## Common Workflows

### Adding a New Tauri Command
1. Define in appropriate `src-tauri/src/*.rs` file with `#[tauri::command]`
2. Register in `src-tauri/src/lib.rs`
3. Create typed wrapper in `src/services/tauri/`
4. Use in React components

### Creating a New Release
1. Run `pnpm publish:new` (auto-bumps patch version)
2. Or manually: update version in `package.json`, `Cargo.toml`, `tauri.conf.json`
3. Run `pnpm tb` to build
4. Use `scripts/release.ts` to upload to GitHub

### Resetting Activation
Delete `activation_status_fingerprint` in the hashed subdirectory within Documents folder, then re-upload fresh `activation_codes.enc` to GitHub Release.
