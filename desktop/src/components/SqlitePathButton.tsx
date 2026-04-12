import { useEffect, useState } from "react";
import { DatabaseIcon, FolderOpenIcon } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";

import { setSqlitePath } from "@/api/commands";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { RuntimeStatus } from "@/types";

export function SqlitePathButton({
  status,
  onSaved,
}: {
  status: RuntimeStatus | null;
  onSaved: () => Promise<void> | void;
}) {
  const [open, setOpen] = useState(false);
  const [sqlitePathInput, setSqlitePathInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    setSqlitePathInput(status?.sqlitePath ?? "");
  }, [status?.sqlitePath]);

  const handlePickSqlitePath = async () => {
    try {
      const selectedPath = await save({
        title: "选择 SQLite 存储文件",
        defaultPath: status?.sqlitePath ?? "reading.sqlite",
        filters: [
          { name: "SQLite Database", extensions: ["sqlite", "db"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });

      if (selectedPath) {
        setSqlitePathInput(selectedPath);
      }
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error));
    }
  };

  const handleSaveSqlitePath = async () => {
    if (!sqlitePathInput.trim() || saving) {
      return;
    }

    setSaving(true);
    setFeedback(null);
    try {
      await setSqlitePath(sqlitePathInput.trim());
      await onSaved();
      setOpen(false);
    } catch (error) {
      setFeedback(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <Button
        variant="outline"
        size="icon"
        className={cn(
          !status?.sqliteConfigured &&
            "border-rose-200 bg-rose-50 text-rose-700 hover:bg-rose-100 hover:text-rose-800 dark:border-rose-900/60 dark:bg-rose-950/30 dark:text-rose-300 dark:hover:bg-rose-950/50",
        )}
        title="SQLite 路径设置"
        aria-label="SQLite 路径设置"
        onClick={() => setOpen(true)}
      >
        <DatabaseIcon className="size-4" />
      </Button>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>SQLite 路径</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            {feedback && (
              <Alert>
                <AlertDescription>{feedback}</AlertDescription>
              </Alert>
            )}
            <div className="flex gap-2">
              <Input
                placeholder="/Users/you/.reading-task/app.sqlite"
                value={sqlitePathInput}
                onChange={(event) => setSqlitePathInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    void handleSaveSqlitePath();
                  }
                }}
              />
              <Button
                type="button"
                variant="outline"
                onClick={() => void handlePickSqlitePath()}
                disabled={saving}
              >
                <FolderOpenIcon className="mr-2 size-4" />
                选择
              </Button>
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => setOpen(false)}
                disabled={saving}
              >
                取消
              </Button>
              <Button onClick={() => void handleSaveSqlitePath()} disabled={saving}>
                {saving ? "保存中…" : "保存"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
