import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import rehypeRaw from "rehype-raw";
import {Eye} from "lucide-react";
import "highlight.js/styles/github-dark.css";

interface MarkdownPreviewProps {
    content: string;
}

export const MarkdownPreview = ({content}: MarkdownPreviewProps) => {
    return (
        <div className="h-full flex flex-col bg-[hsl(var(--preview-bg))]">
            <div className="flex items-center gap-2 px-6 py-4 border-b border-border bg-card/50">
                <Eye className="w-5 h-5 text-primary"/>
                <h2 className="text-lg font-semibold text-gray-900">预览</h2>
            </div>

            <div className="flex-1 overflow-y-auto custom-scrollbar">
                <div className="px-6 py-8 markdown-preview">
                    {content ? (
                        <ReactMarkdown
                            remarkPlugins={[remarkGfm]}
                            rehypePlugins={[rehypeHighlight, rehypeRaw]}
                        >
                            {content}
                        </ReactMarkdown>
                    ) : (
                        <div className="flex items-center justify-center h-full text-gray-600">
                            在左侧输入 Markdown 内容查看预览
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
};
