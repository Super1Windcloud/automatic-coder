import { Textarea } from "@/components/ui/textarea";
import { FileText } from "lucide-react";

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export const MarkdownEditor = ({ value, onChange }: MarkdownEditorProps) => {
  return (
    <div className="h-full flex flex-col border-r border-border bg-[hsl(var(--editor-bg))]">
      <div className="flex items-center gap-2 px-6 py-4 border-b border-border bg-card/50">
        <FileText className="w-5 h-5 text-primary" />
        <h2 className="text-lg font-semibold text-foreground">编辑器</h2>
      </div>

      <Textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="在这里输入 Markdown 内容..."
        className="flex-1 resize-none border-0 rounded-none focus-visible:ring-0 bg-transparent p-6 font-mono text-sm leading-relaxed custom-scrollbar focus-visible:ring-offset-0"
      />
    </div>
  );
};
