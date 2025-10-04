import { useState } from "react";
import { MarkdownEditor } from "@/components/MarkdownEditor";
import { MarkdownPreview } from "@/components/MarkdownPreview";
import { Button } from "@/components/ui/button";
import { Copy, Download, Sparkles } from "lucide-react";
import { toast } from "sonner";

const defaultMarkdown = `# 欢迎使用 Markdown 编辑器

这是一个功能强大的 **Markdown 编辑器**，支持实时预览。

## 主要特性

- ✨ 实时预览
- 🎨 语法高亮
- 📝 GitHub Flavored Markdown
- 💾 复制和导出功能

## 代码示例

\`\`\`javascript
function greet(name) {
  console.log(\`Hello, \${name}!\`);
  return true;
}

greet("World");
\`\`\`

## 表格支持

| 功能 | 状态 | 说明 |
|------|------|------|
| 实时预览 | ✅ | 即时渲染 |
| 代码高亮 | ✅ | 支持多种语言 |
| 导出 | ✅ | Markdown 格式 |

## 列表

### 无序列表
- 第一项
- 第二项
  - 嵌套项
  - 另一个嵌套项
- 第三项

### 有序列表
1. 首先
2. 其次
3. 最后

## 引用

> 这是一段引用文本
> 
> 可以跨多行显示

## 链接和图片

[访问 GitHub](https://github.com)

---

开始编辑左侧的内容，右侧将实时显示渲染效果！
`;

const Index = () => {
  const [markdown, setMarkdown] = useState(defaultMarkdown);

  const handleCopy = () => {
    navigator.clipboard.writeText(markdown);
    toast("已复制", {
      description: "Markdown 内容已复制到剪贴板",
    });
  };

  const handleDownload = () => {
    const blob = new Blob([markdown], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "document.md";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast("下载成功", {
      duration: 3000,
      position: "top-center",
      id: "download-success",
      description: "Markdown 文件已保存",
    });
  };

  return (
    <div className="h-screen flex flex-col bg-background">
      {/* Header */}
      <header className="border-b border-border bg-card/50 backdrop-blur supports-[backdrop-filter]:bg-card/30">
        <div className="flex items-center justify-between px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-primary to-accent flex items-center justify-center shadow-[var(--shadow-glow)]">
              <Sparkles className="w-6 h-6 text-white" />
            </div>
            <div>
              <h1 className="text-xl font-bold bg-gradient-to-r from-primary to-accent bg-clip-text text-transparent">
                Markdown 编辑器
              </h1>
              <p className="text-xs text-muted-foreground">
                实时预览 · 功能强大
              </p>
            </div>
          </div>

          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleCopy}
              className="gap-2"
            >
              <Copy className="w-4 h-4" />
              复制
            </Button>
            <Button
              variant="default"
              size="sm"
              onClick={handleDownload}
              className="gap-2 bg-gradient-to-r from-primary to-accent hover:opacity-90 transition-opacity"
            >
              <Download className="w-4 h-4" />
              导出
            </Button>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <div className="flex-1 grid grid-cols-1 md:grid-cols-2 overflow-hidden">
        <MarkdownEditor value={markdown} onChange={setMarkdown} />
        <MarkdownPreview content={markdown} />
      </div>
    </div>
  );
};

export default Index;
