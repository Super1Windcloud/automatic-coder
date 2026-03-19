## 激活管理

应用在启动前会检查本地激活状态并要求输入加密激活码。后端会从远端仓库下载最新的 `activation_codes.enc`，完成解密与匹配后立即上传更新后的文件，同时只在本机落地一次性指纹。

### 环境变量

- 在 `src-tauri/.env` 或系统环境中设置 `ACTIVATION_MASTER_KEY`，必须是 32 字节密钥，可使用 Base64、Hex 或原始 32 个字符。
- 远端存储使用 Github 发布附件，可通过以下变量自定义，未设置时会使用默认值：
  - `ACTIVATION_REMOTE_OWNER`（默认 `SuperWindcloud`）
  - `ACTIVATION_REMOTE_REPO`（默认 `rust_default_arg`）
  - `ACTIVATION_REMOTE_TAG`（默认 `0.1.0`）
  - `ACTIVATION_REMOTE_TOKEN` 或 `GITEE_TOKEN`：访问令牌，缺省时仅用于开发。

### 生成激活码

1. 进入 `src-tauri` 目录，为密钥生成 10,000 个激活码并写入资产目录：

   ```bash
   cargo run -p license_manager --bin generate -- assets "<ACTIVATION_MASTER_KEY>" 10000 16
   ```

2. 生成的文件说明：
   - `assets/activation_codes.json`：原始激活码（可自行妥善保管或删除）。
   - `assets/activation_codes.enc`：需要上传到远端仓库的加密激活文件。
   - `assets/activation_codes_client.txt`：分发给用户的加密激活码，一行一个。

3. 每次更新激活码后，使用 `scripts/refreshActivationCodes.ts` 中的 `downloadActivateCodeFileAndDeleteAttach` 与 `updateActicationCodeFile` 对发布附件进行下载、消费和回传，确保远端文件永远只有一份最新内容。

### 使用流程

1. 前端弹窗要求用户粘贴加密激活码。
2. 后端拉取远端加密存储并解密，与可用列表匹配。
3. 匹配成功后将激活码从远端存储中移除，上传更新后的 `activation_codes.enc`，并在本机写入激活标记。
4. 激活失败会给出对应提示（无效、已使用或系统未启用）。

激活指纹会写入三个位置，且三者同时存在才视为激活成功：系统“文档”目录、Local AppData（或等效目录）、Roaming AppData（或等效目录）。每个位置都会创建一个以 64 位指纹哈希命名的文件夹，并放置 `activation_status_fingerprint` 文件，便于与常规应用数据区分。

如需重置激活状态，可删除系统“文档”目录下以 64 位指纹命名的子目录中的 `activation_status_fingerprint` 文件，然后重新分发新的远端激活文件。
