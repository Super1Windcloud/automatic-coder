import pkg from "../package.json" with { type: "json" };
import * as fs from "node:fs";
import dotenv from "dotenv";
import axios from "axios";
import FormData from "form-data";
dotenv.config({
  path: "../.env",
});

const owner: string = "SuperWindCloud";
const repo: string = "rust_default_arg";
const path: string = "s3";
const token: string = process.env.GITEE_TOKEN || "";

const templateStr = {
  version: "",
  platforms: {
    "linux-x86_64": {
      signature: "",
      url: "",
    },
    "windows-x86_64": {
      signature: "",
      url: "",
    },
    "darwin-x86_64": {
      signature: "",
      url: "",
    },
  },
};

templateStr.version = pkg.version;

const signPath = `../bundle/nsis/Interview-Coder_${pkg.version}_x64-setup.exe.sig`;

const signContent = fs.readFileSync(signPath, "utf-8");
templateStr.platforms["windows-x86_64"].signature = signContent;
templateStr.platforms["windows-x86_64"].url =
  `https://gitee.com/SuperWindcloud/rust_default_arg/releases/download/${pkg.version}/Interview-Coder_${pkg.version}_x64-setup.exe`;
templateStr.platforms["linux-x86_64"].signature = signContent;
templateStr.platforms["linux-x86_64"].url =
  `https://gitee.com/SuperWindcloud/rust_default_arg/releases/download/${pkg.version}/Interview-Coder_${pkg.version}_x64-setup.exe`;
templateStr.platforms["darwin-x86_64"].signature = signContent;
templateStr.platforms["darwin-x86_64"].url =
  `https://gitee.com/SuperWindcloud/rust_default_arg/releases/download/${pkg.version}/Interview-Coder_${pkg.version}_x64-setup.exe`;

console.log(templateStr);

async function getFileInfo() {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/contents/${path}`;
  const { data } = await axios.get(url, {
    params: token ? { access_token: token } : {},
  });
  return data.sha;
}

async function updateS3File(newContent: string, sha: string) {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/contents/${path}`;
  const message = "publish new version";
  const encodedContent = Buffer.from(newContent, "utf-8").toString("base64");

  const res = await axios.put(url, {
    access_token: token,
    content: encodedContent,
    sha,
    message,
  });

  return res.data;
}

async function uploadAttach(releaseId: number, filePath: string) {
  const formData = new FormData();
  formData.append("file", fs.createReadStream(filePath));

  try {
    const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${releaseId}/attach_files`;

    await axios.post(url, formData, {
      params: {
        access_token: token,
      },
      headers: formData.getHeaders(),
      maxContentLength: Infinity,
      maxBodyLength: Infinity,
    });

    console.log("✅ 上传成功!");
  } catch (err) {
    console.error("❌ 上传失败:", err);
  }
}

async function uploadAttachInstallerAndCreateRelease() {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases`;

  try {
    const res = await axios.post(
      url,
      {
        access_token: token,
        tag_name: pkg.version,
        name: pkg.version,
        body: "enjoy it!",
        target_commitish: "master",
      },
      {
        headers: { "Content-Type": "application/json" },
      },
    );

    console.log("✅ Release 创建成功：", res.data.name);
    return res.data;
  } catch (error: unknown) {
    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
    // @ts-expect-error
    console.error("❌ 创建失败：", error.response?.data || error.message);
  }

  const latestUrl = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/latest`;
  const res = await axios.get(latestUrl, {
    params: token ? { access_token: token } : {},
  });
  const releaseId = res.data.id;
  console.log(releaseId);
  await uploadAttach(
    releaseId,
    `../bundle/nsis/Interview-Coder_${pkg.version}_x64-setup.exe`,
  );
}

(async () => {
  const sha = await getFileInfo();
  console.log(sha);
  const json = JSON.stringify(templateStr, null, 2);
  console.log(json);
  await updateS3File(json, sha);
  await uploadAttachInstallerAndCreateRelease();
})();
