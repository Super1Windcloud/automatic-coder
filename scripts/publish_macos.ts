import * as fs from "node:fs";
import path from "node:path";
import axios from "axios";
import dotenv from "dotenv";
import pkg from "../package.json" with { type: "json" };

dotenv.config({
  path: process.cwd() + "/.env",
});

const owner: string = "Super1Windcloud";
const repo: string = "automatic-coder";
const latestJsonAssetName = "latest.json";
const token: string =
  process.env.GITHUB_TOKEN || process.env.GitHub_token || "";

interface PlatformInfo {
  signature: string;
  url: string;
}

interface Template {
  version: string;
  platforms: Record<string, PlatformInfo>;
}

function pruneEmptyPlatforms(template: Template): Template {
  const platforms = Object.fromEntries(
    Object.entries(template.platforms).filter(([, info]) => {
      return Boolean(info.signature?.trim() && info.url?.trim());
    }),
  );

  return {
    ...template,
    platforms,
  };
}

async function fetchTemplate(): Promise<Template> {
  const url = `https://github.com/${owner}/${repo}/releases/latest/download/${latestJsonAssetName}`;
  try {
    const res = await axios.get(url, {
      headers: {
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
        Accept: "application/json",
        "User-Agent": "Axios-Client",
      },
    });
    return res.data as Template;
  } catch (err) {
    console.warn("❌ Fetching template failed, using default structure.");
    return {
      version: "0.0.0",
      platforms: {
        "windows-x86_64": { signature: "", url: "" },
        "darwin-x86_64": { signature: "", url: "" },
        "darwin-aarch64": { signature: "", url: "" },
      },
    };
  }
}

const templateStr = await fetchTemplate();
templateStr.version = pkg.version;

const signPath = process.cwd() + `/bundle/macos/Interview-Coder.app.tar.gz.sig`;
const signContent = fs.readFileSync(signPath, "utf-8");

if (!templateStr.platforms["darwin-x86_64"]) {
  templateStr.platforms["darwin-x86_64"] = { signature: "", url: "" };
}
templateStr.platforms["darwin-x86_64"].signature = signContent;
templateStr.platforms["darwin-x86_64"].url =
  `https://github.com/${owner}/${repo}/releases/download/${pkg.version}/Interview-Coder.app.tar.gz`;

// Also update aarch64 if it's universal or if you have a separate build
if (!templateStr.platforms["darwin-aarch64"]) {
  templateStr.platforms["darwin-aarch64"] = { signature: "", url: "" };
}
templateStr.platforms["darwin-aarch64"].signature = signContent;
templateStr.platforms["darwin-aarch64"].url =
  `https://github.com/${owner}/${repo}/releases/download/${pkg.version}/Interview-Coder.app.tar.gz`;

console.log(templateStr);

async function uploadAsset(
  uploadUrl: string,
  filePath: string,
  fileName: string,
) {
  const base_url = uploadUrl.split("{")[0];
  const url = `${base_url}?name=${fileName}`;
  const fileData = fs.readFileSync(filePath);

  try {
    await axios.post(url, fileData, {
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/octet-stream",
        "User-Agent": "Axios-Client",
      },
      maxContentLength: Infinity,
      maxBodyLength: Infinity,
    });
    console.log(`✅ Asset ${fileName} 上传成功!`);
  } catch (err) {
    console.error(`❌ Asset ${fileName} 上传失败:`, err);
  }
}

function getMacosDmgPath() {
  const dmgDir = process.cwd() + "/bundle/dmg";
  const dmgFile = fs
    .readdirSync(dmgDir)
    .find((file) => file.endsWith(".dmg"));

  if (!dmgFile) {
    throw new Error("No dmg file found under bundle/dmg");
  }

  if (!dmgFile.includes(pkg.version)) {
    throw new Error(
      `DMG file name must include version ${pkg.version}, got ${dmgFile}`,
    );
  }

  return {
    fileName: dmgFile,
    filePath: `${dmgDir}/${dmgFile}`,
  };
}

async function getOrCreateRelease() {
  const releasesUrl = `https://api.github.com/repos/${owner}/${repo}/releases`;
  try {
    const tagUrl = `${releasesUrl}/tags/${pkg.version}`;
    const { data: existingRelease } = await axios.get(tagUrl, {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github.v3+json",
        "User-Agent": "Axios-Client",
      },
    });
    return existingRelease;
  } catch (err) {
    const res = await axios.post(
      releasesUrl,
      {
        tag_name: pkg.version,
        name: pkg.version,
        body: "enjoy it!",
        draft: false,
        prerelease: false,
      },
      {
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github.v3+json",
          "User-Agent": "Axios-Client",
        },
      },
    );
    return res.data;
  }
}

async function deleteExistingAsset(release: any, fileName: string) {
  const asset = release.assets?.find((a: any) => a.name === fileName);
  if (asset) {
    console.log(`❌ 已存在 ${fileName}，删除...`);
    const url = `https://api.github.com/repos/${owner}/${repo}/releases/assets/${asset.id}`;
    await axios.delete(url, {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github.v3+json",
        "User-Agent": "Axios-Client",
      },
    });
  }
}

(async () => {
  const sanitizedTemplate = pruneEmptyPlatforms(templateStr);
  const json = JSON.stringify(sanitizedTemplate, null, 2);
  const release = await getOrCreateRelease();
  const latestJsonFilePath = path.join(process.cwd(), latestJsonAssetName);
  fs.writeFileSync(latestJsonFilePath, `${json}\n`, "utf-8");

  await deleteExistingAsset(release, latestJsonAssetName);
  await uploadAsset(release.upload_url, latestJsonFilePath, latestJsonAssetName);

  const updaterFileName = `Interview-Coder.app.tar.gz`;
  await deleteExistingAsset(release, updaterFileName);
  await uploadAsset(
    release.upload_url,
    process.cwd() + `/bundle/macos/${updaterFileName}`,
    updaterFileName,
  );

  const dmgAsset = getMacosDmgPath();
  await deleteExistingAsset(release, dmgAsset.fileName);
  await uploadAsset(release.upload_url, dmgAsset.filePath, dmgAsset.fileName);
})();
