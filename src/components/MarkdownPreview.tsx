import MarkdownPreviewBase from "@uiw/react-markdown-preview";

export function MarkdownPreview({ content }: { content: string }) {
  return (
    <MarkdownPreviewBase
      source={content}
      style={{
        padding: 16,
        scrollbarWidth: "none",
        backgroundColor: "transparent",
      }}
    />
  );
}
