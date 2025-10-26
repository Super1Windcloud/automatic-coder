import { CodeLanguage } from "@/store";

export const defaultMarkdown = `# 欢迎使用 Markdown 编辑器

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

export const getLLMPrompts = (language: CodeLanguage) =>
  `你是一名参加秋招笔试的程序员,图中可能有选择题和算法题,如果是选择题请直接给出答案和简要解析,如果是算法题请使用${language}完成图中的算法题,给出代码和解题思路`;

export  const templatePattern = /^你是一名参加秋招笔试的程序员,图中可能有选择题和算法题,如果是选择题请直接给出答案和简要解析,如果是算法题请使用(.+?)完成图中的算法题,给出代码和解题思路$/;
