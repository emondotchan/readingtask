import { useState } from "react";
import {
  AlertTriangleIcon,
  BookOpenIcon,
  CheckCircle2Icon,
  ChevronRightIcon,
  KeyIcon,
  ShieldAlertIcon,
  StoreIcon,
  UsersIcon,
  type LucideIcon,
} from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import type { RuntimeStatus } from "@/types";
import { CourseManager } from "./CourseManager";
import { FcManager } from "./FcManager";
import { OpenIdManager } from "./OpenIdManager";
import { ShopManager } from "./ShopManager";

interface Props {
  status: RuntimeStatus | null;
  error: string | null;
  onRuntimeStatusChanged: () => Promise<void> | void;
}

interface ConfigCardProps {
  title: string;
  summary: string;
  ready: boolean;
  icon: LucideIcon;
  onClick: () => void;
}

function ConfigCard({ title, summary, ready, icon: Icon, onClick }: ConfigCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group flex h-full flex-col rounded-xl border bg-card p-4 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:shadow-md",
        ready
          ? "border-border hover:border-primary/30"
          : "border-rose-200/80 hover:border-rose-300 dark:border-rose-900/50 dark:hover:border-rose-800",
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div
          className={cn(
            "inline-flex size-10 shrink-0 items-center justify-center rounded-xl border transition-colors",
            ready
              ? "border-border bg-muted/60 text-muted-foreground group-hover:bg-primary/10 group-hover:text-primary"
              : "border-rose-200 bg-rose-50 text-rose-600 dark:border-rose-900/60 dark:bg-rose-950/30 dark:text-rose-300",
          )}
        >
          <Icon className="size-5" />
        </div>
        <Badge
          variant="outline"
          className={cn(
            "shrink-0 rounded-full px-2.5 py-1 text-[11px] font-medium",
            ready
              ? "border-emerald-200 bg-emerald-50/70 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/20 dark:text-emerald-300"
              : "border-rose-200 bg-rose-50/70 text-rose-700 dark:border-rose-900 dark:bg-rose-950/20 dark:text-rose-300",
          )}
        >
          {ready ? (
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

      <div className="mt-4 space-y-1">
        <p className="text-sm font-semibold tracking-tight text-foreground">{title}</p>
        <p className="text-xs leading-[1.1rem] text-muted-foreground">{summary}</p>
      </div>

      <div className="mt-4 flex items-center justify-between border-t border-border/60 pt-2.5 text-xs text-muted-foreground">
        <span>{ready ? "点击管理" : "点击补齐"}</span>
        <ChevronRightIcon className="size-4 transition-transform group-hover:translate-x-0.5" />
      </div>
    </button>
  );
}

export default function ConfigStatus({
  status,
  error,
  onRuntimeStatusChanged,
}: Props) {
  const [courseManagerOpen, setCourseManagerOpen] = useState(false);
  const [openIdManagerOpen, setOpenIdManagerOpen] = useState(false);
  const [shopManagerOpen, setShopManagerOpen] = useState(false);
  const [fcManagerOpen, setFcManagerOpen] = useState(false);

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
        <Skeleton className="h-[108px] rounded-lg" />
        <Skeleton className="h-[108px] rounded-lg" />
        <Skeleton className="h-[108px] rounded-lg" />
        <Skeleton className="h-[108px] rounded-lg" />
      </div>
    );
  }

  const allReady =
    status.sqliteConfigured &&
    status.openIdsReady &&
    status.shopReady &&
    status.provinceReady &&
    status.fcReady &&
    status.courseReady;

  return (
    <>
      <div className="grid gap-4 sm:grid-cols-4">
        <ConfigCard
          title="课程配置"
          summary="管理月份、课程 ID 与任务类型"
          ready={status.courseReady}
          icon={BookOpenIcon}
          onClick={() => {
            if (status.sqliteConfigured) {
              setCourseManagerOpen(true);
            }
          }}
        />
        <ConfigCard
          title="OpenID 数据"
          summary="管理执行任务所需的 OpenID"
          ready={status.openIdsReady}
          icon={KeyIcon}
          onClick={() => {
            if (status.sqliteConfigured) {
              setOpenIdManagerOpen(true);
            }
          }}
        />
        <ConfigCard
          title="门店架构映射"
          summary="管理门店、省市与 FC 对应关系"
          ready={status.shopReady}
          icon={StoreIcon}
          onClick={() => {
            if (status.sqliteConfigured) {
              setShopManagerOpen(true);
            }
          }}
        />
        <ConfigCard
          title="FC 配置"
          summary="管理 FC 名称"
          ready={status.fcReady}
          icon={UsersIcon}
          onClick={() => {
            if (status.sqliteConfigured) {
              setFcManagerOpen(true);
            }
          }}
        />
      </div>

      {!allReady && (
        <Alert className="border-amber-200/80 bg-amber-50/90 text-amber-800 shadow-sm dark:border-amber-900/50 dark:bg-amber-950/25 dark:text-amber-200">
          <AlertTriangleIcon className="size-5" />
          <AlertTitle>运行环境未准备就绪</AlertTitle>
          <AlertDescription>
            {!status.sqliteConfigured
              ? "SQLite 运行环境尚未初始化，请先在应用外完成配置。"
              : "请点击上方卡片，补齐缺失的配置数据后再创建或执行任务。"}
          </AlertDescription>
        </Alert>
      )}

      <CourseManager
        open={courseManagerOpen}
        onOpenChange={setCourseManagerOpen}
        onCoursesChanged={onRuntimeStatusChanged}
      />
      <OpenIdManager open={openIdManagerOpen} onOpenChange={setOpenIdManagerOpen} />
      <ShopManager open={shopManagerOpen} onOpenChange={setShopManagerOpen} />
      <FcManager open={fcManagerOpen} onOpenChange={setFcManagerOpen} />
    </>
  );
}
