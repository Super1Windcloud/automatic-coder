# Automatic Coder

An intelligent desktop application for developers, featuring screen capture, AI-driven solutions, and a secure activation system.

## 🚀 Features

- **Screen Capture**: Easily capture snippets of your screen for analysis.
- **AI Solutions**: Integrated with VLM (Vision Language Models) to provide real-time coding assistance and problem-solving.
- **Secure Activation**: Robust licensing system hosted on GitHub to manage application access.
- **Auto-Updates**: Seamless background updates powered by Tauri and GitHub Releases.

## 🔑 License Management

The application validates the local activation status upon startup. If not activated, it requires an encrypted activation code. The backend fetches the latest `activation_codes.enc` from the remote GitHub repository, decrypts and matches the code, and then uploads the updated file while persisting a one-time machine fingerprint locally.

### Generating Activation Codes

1. Navigate to the `src-tauri` directory and generate activation codes (e.g., 10,000 codes) for your master key:

   ```bash
   cargo run -p license_manager --bin generate -- assets "<YOUR_ACTIVATION_MASTER_KEY>" 10000 16
   ```

2. Generated Files:
   - `assets/activation_codes.json`: Raw activation codes (keep secure or delete after use).
   - `assets/activation_codes.enc`: Encrypted activation file to be uploaded to the GitHub Release assets.
   - `assets/activation_codes_client.txt`: Encrypted codes to be distributed to users (one per line).

3. **Remote Sync**: Use the scripts in `scripts/refreshActivationCodes.ts` (specifically `downloadActivateCodeFileAndDeleteAttach` and `updateActivationCodeFile`) to download, consume, and re-upload the activation file to GitHub, ensuring only the latest version exists remotely.

### Activation Workflow

1. **User Input**: A prompt appears asking for the encrypted activation code.
2. **Remote Verification**: The backend pulls the encrypted store from GitHub, decrypts it, and checks against the provided code.
3. **Consumption**: Upon a successful match, the code is removed from the remote store, the updated `activation_codes.enc` is re-uploaded, and an activation flag is written locally.
4. **Feedback**: Clear status messages are provided (Invalid, Already Used, or System Disabled).

### Machine Fingerprinting

The activation fingerprint is stored in three distinct locations. Activation is only considered valid if all three exist:

- System "Documents" directory.
- Local AppData (or platform equivalent).
- Roaming AppData (or platform equivalent).

Each location contains a folder named after a 64-bit fingerprint hash containing an `activation_status_fingerprint` file.

**To Reset Activation**: Delete the `activation_status_fingerprint` file within the hashed subdirectory in the "Documents" folder, then re-distribute a fresh remote activation file.

## 🛠 Development

### Prerequisites

- [Rust](https://www.rust-lang.org/)
- [Node.js](https://nodejs.org/) (pnpm recommended)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/setup/)

### Setup

1. Install dependencies:

   ```bash
   pnpm install
   ```

2. Configure environment variables in `src-tauri/.env`:

   ```env
   ACTIVATION_MASTER_KEY=your_key
   GITHUB_TOKEN=your_github_pat
   GITHUB_OWNER=Super1Windcloud
   GITHUB_REPO=automatic-coder
   GITHUB_RELEASE_TAG=v1.0.0
   ```

3. Run in development mode:
   ```bash
   pnpm tauri dev
   ```

## 📦 Publishing

Use the provided scripts to publish new versions to GitHub:

- Automatic bump + build + upload:
  `pnpm publish:new`
- Explicit platform:
  `pnpm publish:new:windows`
  `pnpm publish:new:macos`

- **Windows**: `pnpm tsx scripts/publish_windows.ts`
- **macOS**: `pnpm tsx scripts/publish_macos.ts`

`publish:new` will:

1. Increment the patch version automatically, for example `1.0.11 -> 1.0.12`.
2. Sync the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
3. Run `pnpm tb` to build the installer bundle.
4. Upload the generated assets to the GitHub Release for that version.

The platform-specific scripts will:

1. Update `latest.json` with the new version and signatures.
2. Create/Update a GitHub Release.
3. Upload the installer artifacts and signatures as release assets.

## 📄 License

This project is licensed under the [LICENSE](LICENSE) file.
