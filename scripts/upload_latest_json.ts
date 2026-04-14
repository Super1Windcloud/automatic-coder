import * as fs from "node:fs";
import path from "node:path";
import axios from "axios";
import dotenv from "dotenv";
import pkg from "../package.json" with { type: "json" };

dotenv.config({
  path: process.cwd() + "/.env",
});

type SupportedPlatform = "windows" | "macos";

interface PlatformInfo {
  signature: string;
  url: string;
}

interface Template {
  version: string;
  platforms: Record<string, PlatformInfo>;
}

const owner = "Super1Windcloud";
const repo = "automatic-coder";
const latestJsonAssetName = "latest.json";
const token = process.env.GITHUB_TOKEN || process.env.GitHub_token || "";

function resolvePlatform(): SupportedPlatform {
  const arg = process.argv[2]?.toLowerCase();
  if (arg === "windows" || arg === "macos") {
    return arg;
  }

  if (process.platform === "win32") {
    return "windows";
  }

  if (process.platform === "darwin") {
    return "macos";
  }

  throw new Error('Unsupported platform. Pass "windows" or "macos" explicitly.');
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
  } catch {
    return {
      version: "0.0.0",
      platforms: {},
    };
  }
}

function pruneEmptyPlatforms(template: Template): Template {
  const platforms = Object.fromEntries(
    Object.entries(template.platforms).filter(([, info]) => {
      return Boolean(info.signature?.trim() && info.url?.trim());
    }),
  );

  return { ...template, platforms };
}

function buildWindowsInfo(template: Template) {
  const signPath = path.join(
    process.cwd(),
    "bundle",
    "nsis",
    `Interview-Coder_${pkg.version}_x64-setup.exe.sig`,
  );
  const signature = fs.readFileSync(signPath, "utf-8");
  template.platforms["windows-x86_64"] = {
    signature,
    url: `https://github.com/${owner}/${repo}/releases/download/${pkg.version}/Interview-Coder_${pkg.version}_x64-setup.exe`,
  };
}

function buildMacosInfo(template: Template) {
  const signPath = path.join(
    process.cwd(),
    "bundle",
    "macos",
    "Interview-Coder.app.tar.gz.sig",
  );
  const signature = fs.readFileSync(signPath, "utf-8");
  const url = `https://github.com/${owner}/${repo}/releases/download/${pkg.version}/Interview-Coder.app.tar.gz`;

  template.platforms["darwin-x86_64"] = { signature, url };
  template.platforms["darwin-aarch64"] = { signature, url };
}

async function getReleaseByTag() {
  const tagUrl = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${pkg.version}`;
  const { data } = await axios.get(tagUrl, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github.v3+json",
      "User-Agent": "Axios-Client",
    },
  });
  return data;
}

async function deleteExistingAsset(release: any, fileName: string) {
  const asset = release.assets?.find((a: any) => a.name === fileName);
  if (!asset) {
    return;
  }

  const url = `https://api.github.com/repos/${owner}/${repo}/releases/assets/${asset.id}`;
  await axios.delete(url, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github.v3+json",
      "User-Agent": "Axios-Client",
    },
  });
}

async function uploadAsset(uploadUrl: string, filePath: string, fileName: string) {
  const baseUrl = uploadUrl.split("{")[0];
  const url = `${baseUrl}?name=${fileName}`;
  const fileData = fs.readFileSync(filePath);

  await axios.post(url, fileData, {
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/octet-stream",
      "User-Agent": "Axios-Client",
    },
    maxContentLength: Infinity,
    maxBodyLength: Infinity,
  });
}

async function main() {
  if (!token) {
    throw new Error("Missing GITHUB_TOKEN or GitHub_token in .env");
  }

  const platform = resolvePlatform();
  const template = await fetchTemplate();
  template.version = pkg.version;

  if (platform === "windows") {
    buildWindowsInfo(template);
  } else {
    buildMacosInfo(template);
  }

  const sanitizedTemplate = pruneEmptyPlatforms(template);
  const latestJsonFilePath = path.join(process.cwd(), latestJsonAssetName);
  fs.writeFileSync(
    latestJsonFilePath,
    `${JSON.stringify(sanitizedTemplate, null, 2)}\n`,
    "utf-8",
  );

  const release = await getReleaseByTag();
  await deleteExistingAsset(release, latestJsonAssetName);
  await uploadAsset(release.upload_url, latestJsonFilePath, latestJsonAssetName);
  console.log(`Uploaded ${latestJsonAssetName} for ${platform} ${pkg.version}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
