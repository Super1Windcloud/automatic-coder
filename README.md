# Interview-coder
次世代程序员笔试助手

## 激活管理

应用在启动前会检查本地激活状态并要求输入加密激活码。后端会对激活码进行解密、匹配及销毁，未通过验证无法进入主界面。

### 环境变量

- 在 `src-tauri/.env` 或系统环境中设置 `ACTIVATION_MASTER_KEY`，必须是 32 字节密钥，可使用 Base64、Hex 或原始 32 个字符。

### 生成激活码

1. 进入 `src-tauri` 目录，为密钥生成 10,000 个激活码并写入资产目录：

   ```bash
   cargo run -p license_manager --bin generate -- assets "<ACTIVATION_MASTER_KEY>" 10000 16
   ```

2. 生成的文件说明：
   - `assets/activation_codes.json`：原始激活码（可自行妥善保管或删除）。
   - `assets/activation_codes.enc`：随安装包发布的加密激活文件，启动时会复制到应用数据目录并进行校验。
   - `assets/activation_codes_client.txt`：分发给用户的加密激活码，一行一个。

3. 构建或发布时确保 `activation_codes.enc` 被包含在安装包的 `assets` 目录中。首次启动时会将该文件复制到应用数据目录。

### 使用流程

1. 前端弹窗要求用户粘贴加密激活码。
2. 后端解密并与存储列表匹配。
3. 匹配成功后将激活码从存储中移除，并在本机写入激活标记。
4. 激活失败会给出对应提示（无效、已使用或系统未启用）。

如需重置激活状态，可删除应用数据目录中的 `activation_codes.enc`（完成备份后）和 `activation_status.json`，然后重新放置新的激活文件。

