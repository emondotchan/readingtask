import { useEffect, useState } from "react";
import {
  AlertTriangleIcon,
  CheckCircle2Icon,
  DatabaseIcon,
  KeyIcon,
  ShieldAlertIcon,
  StoreIcon,
  UsersIcon,
} from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import type { RuntimeStatus } from "@/types";
import { setSqlitePath } from "@/api/commands";
import { save } from "@tauri-apps/plugin-dialog";
import { OpenIdManager } from "./OpenIdManager";
import { ShopManager } from "./ShopManager";
import { FcManager } from "./FcManager";

interface Props {
  status: RuntimeStatus | null;
  error: string | null;
  onRuntimeStatusChanged: () => Promise<void> | void;
}

export default function ConfigStatus({
  status,
  error,
  onRuntimeStatusChanged,
}: Props) {
  const [openIdManagerOpen, setOpenIdManagerOpen] = useState(false);
  const [shopManagerOpen, setShopManagerOpen] = useState(false);
  const [fcManagerOpen, setFcManagerOpen] = useState(false);
  const [sqliteDialogOpen, setSqliteDialogOpen] = useState(false);
  const [sqlitePathInput, setSqlitePathInput] = useState("");
  const [sqliteSaving, setSqliteSaving] = useState(false);
  const [sqliteFeedback, setSqliteFeedback] = useState<string | null>(null);

  useEffect(() => {
    setSqlitePathInput(status?.sqlitePath ?? "");
  }, [status?.sqlitePath]);

  if (error) {
    return (
      <Alert
        variant="destructive"
        className="border-rose-200/80 bg-rose-50/90 text-rose-700 shadow-sm dark:border-rose-900/50 dark:bg-rose-950/30 dark:text-rose-200"
      >
        <ShieldAlertIcon className="size-5" />
        <AlertTitle>无法获取运行时状态</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    );
  }

  if (!status) {
    return (
      <div className="grid gap-4 sm:grid-cols-4">
        <Skeleton className="h-[120px] rounded-lg" />
        <Skeleton className="h-[120px] rounded-lg" />
        <Skeleton className="h-[120px] rounded-lg" />
        <Skeleton className="h-[120px] rounded-lg" />
      </div>
    );
  }

  const allReady =
    status.sqliteConfigured &&
    status.openIdsReady &&
    status.shopReady &&
    status.provinceReady &&
    status.fcReady;

  const handleSaveSqlitePath = async () => {
    if (!sqlitePathInput.trim() || sqliteSaving) {
      return;
    }

    setSqliteSaving(true);
    setSqliteFeedback(null);
    try {
      await setSqlitePath(sqlitePathInput.trim());
      await onRuntimeStatusChanged();
      setSqliteFeedback("SQLite 存储路径已保存");
      setSqliteDialogOpen(false);
    } catch (reason) {
      setSqliteFeedback(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSqliteSaving(false);
    }
  };

  const handlePickSqlitePath = async () => {
    try {
      const selectedPath = await save({
        title: "选择 SQLite 存储文件",
        defaultPath: status.sqlitePath ?? "reading.sqlite",
        filters: [
          { name: "SQLite Database", extensions: ["sqlite", "db"] },
          { name: "All Files", extensions: ["*"] },
        ],
      });

      if (selectedPath) {
        setSqlitePathInput(selectedPath);
      }
    } catch (reason) {
      setSqliteFeedback(reason instanceof Error ? reason.message : String(reason));
    }
  };

  return (
    <>
      <div className="grid gap-4 sm:grid-cols-4">
        <div
          onClick={() => setSqliteDialogOpen(true)}
          className={cn(
            "group flex cursor-pointer flex-col justify-between rounded-lg border bg-card p-5 shadow-sm transition-all hover:border-primary/50 hover:shadow-md",
            status.sqliteConfigured
              ? "border-border"
              : "border-rose-200/80 dark:border-rose-900/50",
          )}
        >
          <div className="flex items-start justify-between">
            <div
              className={cn(
                "inline-flex size-10 items-center justify-center rounded-md transition-colors",
                status.sqliteConfigured
                  ? "bg-muted text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary"
                  : "bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-300",
              )}
            >
              <DatabaseIcon className="size-5" />
            </div>
            <Badge
              variant="outline"
              className={
                status.sqliteConfigured
                  ? "text-emerald-700 border-emerald-200 bg-emerald-50/50 dark:bg-emerald-950/20 dark:border-emerald-900"
                  : "text-rose-700 border-rose-200 bg-rose-50/50 dark:bg-rose-950/20 dark:border-rose-900"
              }
            >
              {status.sqliteConfigured ? (
                <>
                  <CheckCircle2Icon className="mr-1 size-3" />
                  已配置
                </>
              ) : (
                <>
                  <AlertTriangleIcon className="mr-1 size-3" />
                  未配置
                </>
              )}
            </Badge>
          </div>
          <div className="mt-4">
            <p className="text-sm font-semibold tracking-tight text-foreground">
              SQLite 存储
            </p>
            <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
              {status.sqlitePath || "点击配置 SQLite 数据库存储文件路径"}
            </p>
          </div>
        </div>

        <div
          onClick={() =>
            status.sqliteConfigured
              ? setOpenIdManagerOpen(true)
              : setSqliteDialogOpen(true)
          }
          className={cn(
            "group flex cursor-pointer flex-col justify-between rounded-lg border bg-card p-5 shadow-sm transition-all hover:border-primary/50 hover:shadow-md",
            status.openIdsReady
              ? "border-border"
              : "border-rose-200/80 dark:border-rose-900/50",
          )}
        >
          <div className="flex items-start justify-between">
            <div
              className={cn(
                "inline-flex size-10 items-center justify-center rounded-md transition-colors",
                status.openIdsReady
                  ? "bg-muted text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary"
                  : "bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-300",
              )}
            >
              <KeyIcon className="size-5" />
            </div>
            <Badge
              variant="outline"
              className={
                status.openIdsReady
                  ? "text-emerald-700 border-emerald-200 bg-emerald-50/50 dark:bg-emerald-950/20 dark:border-emerald-900"
                  : "text-rose-700 border-rose-200 bg-rose-50/50 dark:bg-rose-950/20 dark:border-rose-900"
              }
            >
              {status.openIdsReady ? (
                <>
                  <CheckCircle2Icon className="mr-1 size-3" />
                  已配置
                </>
              ) : (
                <>
                  <AlertTriangleIcon className="mr-1 size-3" />
                  缺失
                </>
              )}
            </Badge>
          </div>
          <div className="mt-4">
            <p className="text-sm font-semibold tracking-tight text-foreground">
              OpenID 数据
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              管理执行任务所需的 OpenID 凭证
            </p>
          </div>
        </div>

        <div
          onClick={() =>
            status.sqliteConfigured
              ? setShopManagerOpen(true)
              : setSqliteDialogOpen(true)
          }
          className={cn(
            "group flex cursor-pointer flex-col justify-between rounded-lg border bg-card p-5 shadow-sm transition-all hover:border-primary/50 hover:shadow-md",
            status.shopReady
              ? "border-border"
              : "border-rose-200/80 dark:border-rose-900/50",
          )}
        >
          <div className="flex items-start justify-between">
            <div
              className={cn(
                "inline-flex size-10 items-center justify-center rounded-md transition-colors",
                status.shopReady
                  ? "bg-muted text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary"
                  : "bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-300",
              )}
            >
              <StoreIcon className="size-5" />
            </div>
            <Badge
              variant="outline"
              className={
                status.shopReady
                  ? "text-emerald-700 border-emerald-200 bg-emerald-50/50 dark:bg-emerald-950/20 dark:border-emerald-900"
                  : "text-rose-700 border-rose-200 bg-rose-50/50 dark:bg-rose-950/20 dark:border-rose-900"
              }
            >
              {status.shopReady ? (
                <>
                  <CheckCircle2Icon className="mr-1 size-3" />
                  已配置
                </>
              ) : (
                <>
                  <AlertTriangleIcon className="mr-1 size-3" />
                  缺失
                </>
              )}
            </Badge>
          </div>
          <div className="mt-4">
            <p className="text-sm font-semibold tracking-tight text-foreground">
              门店架构映射
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              管理终端门店与省市的归属对应关系
            </p>
          </div>
        </div>

        <div
          onClick={() =>
            status.sqliteConfigured
              ? setFcManagerOpen(true)
              : setSqliteDialogOpen(true)
          }
          className={cn(
            "group flex cursor-pointer flex-col justify-between rounded-lg border bg-card p-5 shadow-sm transition-all hover:border-primary/50 hover:shadow-md",
            status.fcReady
              ? "border-border"
              : "border-rose-200/80 dark:border-rose-900/50",
          )}
        >
          <div className="flex items-start justify-between">
            <div
              className={cn(
                "inline-flex size-10 items-center justify-center rounded-md transition-colors",
                status.fcReady
                  ? "bg-muted text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary"
                  : "bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-300",
              )}
            >
              <UsersIcon className="size-5" />
            </div>
            <Badge
              variant="outline"
              className={
                status.fcReady
                  ? "text-emerald-700 border-emerald-200 bg-emerald-50/50 dark:bg-emerald-950/20 dark:border-emerald-900"
                  : "text-rose-700 border-rose-200 bg-rose-50/50 dark:bg-rose-950/20 dark:border-rose-900"
              }
            >
              {status.fcReady ? (
                <>
                  <CheckCircle2Icon className="mr-1 size-3" />
                  已配置
                </>
              ) : (
                <>
                  <AlertTriangleIcon className="mr-1 size-3" />
                  缺失
                </>
              )}
            </Badge>
          </div>
          <div className="mt-4">
            <p className="text-sm font-semibold tracking-tight text-foreground">
              FC 经理配置
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              管理区域 FC 经理及其 Manager ID
            </p>
          </div>
        </div>
      </div>

      {sqliteFeedback && (
        <Alert className="border-sky-200/80 bg-sky-50/90 text-sky-800 shadow-sm dark:border-sky-900/50 dark:bg-sky-950/25 dark:text-sky-200">
          <AlertTitle>SQLite 配置</AlertTitle>
          <AlertDescription>{sqliteFeedback}</AlertDescription>
        </Alert>
      )}

      {!allReady && (
        <Alert className="border-amber-200/80 bg-amber-50/90 text-amber-800 shadow-sm dark:border-amber-900/50 dark:bg-amber-950/25 dark:text-amber-200">
          <AlertTriangleIcon className="size-5" />
          <AlertTitle>运行环境未准备就绪</AlertTitle>
          <AlertDescription>
            {!status.sqliteConfigured
              ? "请先配置 SQLite 存储文件路径。首次完成配置后，应用才会加载其余数据。"
              : "请点击上方卡片，补齐缺失的配置数据后再创建或执行任务。"}
          </AlertDescription>
        </Alert>
      )}

      <Dialog open={sqliteDialogOpen} onOpenChange={setSqliteDialogOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>配置 SQLite 存储文件路径</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
                disabled={sqliteSaving}
              >
                选择文件
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              支持直接选择 SQLite 文件路径。首次保存时会初始化数据库并导入内置基础配置；后续启动将记住这个路径。
            </p>
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => setSqliteDialogOpen(false)}
                disabled={sqliteSaving}
              >
                取消
              </Button>
              <Button onClick={() => void handleSaveSqlitePath()} disabled={sqliteSaving}>
                {sqliteSaving ? "保存中…" : "保存路径"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <OpenIdManager open={openIdManagerOpen} onOpenChange={setOpenIdManagerOpen} />
      <ShopManager open={shopManagerOpen} onOpenChange={setShopManagerOpen} />
      <FcManager open={fcManagerOpen} onOpenChange={setFcManagerOpen} />
    </>
  );
}
