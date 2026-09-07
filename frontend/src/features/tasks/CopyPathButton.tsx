import { Button } from "@/components/ui/button";

export function CopyPathButton({ path }: { path: string }) {
  return (
    <Button
      type="button"
      variant="ghost"
      className="copy-path"
      aria-label={`复制完整路径 ${path}`}
      onClick={() => void navigator.clipboard?.writeText(path)}
    >
      复制
    </Button>
  );
}
