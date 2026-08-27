import {
  CheckCircle2Icon,
  AlertCircleIcon,
  ActivityIcon,
  PlayIcon,
  Clock3Icon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import type { CommandError, TaskItemResult, TaskRunSummary } from "@/types";

interface Props {
  runState: string;
  processedCount: number;
  requestedCount: number;
  items: TaskItemResult[];
  summary: TaskRunSummary | null;
  error: CommandError | null;
}

export default function QuickRunResults({
  runState,
  processedCount,
  requestedCount,
  items,
  summary,
  error,
}: Props) {
  const getResultSucceeded = (item: TaskItemResult) =>
    item.submit_err === 0
      ? true
      : typeof item.submit_err === "number"
        ? false
        : item.outcome === "Success";
  const getResultRtnMsg = (item: TaskItemResult) => item.rtn_msg ?? item.response_text ?? "—";
  const getResultReadId = (item: TaskItemResult) => item.read_id ?? "None";
  const isRunning = runState === "running";
  const displayProcessedCount = processedCount;
  const displayRequestedCount = requestedCount;
  const progressValue =
    displayRequestedCount > 0 ? (displayProcessedCount / displayRequestedCount) * 100 : 0;
  const displayItems = summary?.items ?? items;

  if (runState === "idle" && displayItems.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center rounded-xl border-2 border-dashed border-border p-12 text-center text-muted-foreground">
        <div className="mb-4 flex size-12 items-center justify-center rounded-full bg-muted">
          <PlayIcon className="size-6 opacity-20" />
        </div>
        <p className="text-sm font-medium">等待任务启动</p>
        <p className="text-xs mt-1 opacity-70">在左侧填写参数并点击开始执行</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col gap-4">
      <div
        className={cn(
          "rounded-xl border p-5 transition-all shadow-sm",
          isRunning
            ? "border-sky-200/70 bg-sky-500/8 dark:border-sky-900/60 dark:bg-sky-950/20"
            : "border-border bg-card",
        )}
      >
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <ActivityIcon
              className={cn(
                "size-4",
                isRunning
                  ? "animate-pulse text-sky-600 dark:text-sky-400"
                  : "text-muted-foreground",
              )}
            />
            <h3 className="text-sm font-semibold text-foreground">执行状态</h3>
          </div>
          <Badge
            variant={isRunning ? "default" : "outline"}
            className={cn("font-mono text-[10px]", isRunning && "bg-sky-500")}
          >
            {displayProcessedCount} / {displayRequestedCount}
          </Badge>
        </div>

        <Progress value={progressValue} className="h-1.5 bg-muted" />

        {error && (
          <div className="mt-4 flex items-start gap-3 rounded-lg border border-rose-200 bg-rose-50 p-3 dark:border-rose-900/50 dark:bg-rose-950/25">
            <AlertCircleIcon className="mt-0.5 size-4 shrink-0 text-rose-500 dark:text-rose-300" />
            <p className="text-xs leading-relaxed font-medium text-rose-700 dark:text-rose-200">
              {error.message}
            </p>
          </div>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-border bg-card shadow-sm">
        <div className="flex items-center justify-between border-b border-border bg-muted/40 px-4 py-3">
          <div className="flex items-center gap-2">
            <Clock3Icon className="size-3.5 text-muted-foreground" />
            <span className="text-[11px] font-bold uppercase tracking-wider text-muted-foreground">
              最近执行记录
            </span>
          </div>
          {isRunning && <Spinner className="size-3 text-sky-500" />}
        </div>

        <ScrollArea className="flex-1">
          <div className="divide-y divide-border">
            {displayItems.length > 0 && (
              <div className="grid grid-cols-[96px_minmax(0,1fr)_92px] gap-3 border-b border-border bg-muted/30 px-4 py-2 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                <span>ShopCode</span>
                <span>RtnMsg</span>
                <span>ReadID</span>
              </div>
            )}
            {displayItems.length === 0 ? (
              <div className="p-8 text-center text-xs italic text-muted-foreground">
                暂无明细记录
              </div>
            ) : (
              [...displayItems].reverse().map((item, idx) => {
                const isSuccess = getResultSucceeded(item);
                return (
                  <div
                    key={idx}
                    className="grid grid-cols-[96px_minmax(0,1fr)_92px] items-center gap-3 px-4 py-3 transition-colors hover:bg-muted/40"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <div
                        className={cn(
                          "size-6 rounded-full flex items-center justify-center shrink-0",
                          isSuccess
                            ? "bg-emerald-50 text-emerald-600 dark:bg-emerald-950/30 dark:text-emerald-300"
                            : "bg-rose-50 text-rose-600 dark:bg-rose-950/30 dark:text-rose-300",
                        )}
                      >
                        {isSuccess ? (
                          <CheckCircle2Icon className="size-3" />
                        ) : (
                          <AlertCircleIcon className="size-3" />
                        )}
                      </div>
                      <div className="min-w-0">
                        <span className="block truncate text-xs font-mono font-medium text-foreground">
                          {item.shop_code}
                        </span>
                        <span className="block text-[10px] font-mono text-muted-foreground">
                          #{item.index}
                        </span>
                      </div>
                    </div>
                    <div className="min-w-0">
                      <p className="truncate text-[10px] text-muted-foreground">
                        {getResultRtnMsg(item)}
                      </p>
                    </div>
                    <div className="min-w-0">
                      <span className="block truncate text-[10px] font-mono text-muted-foreground">
                        {getResultReadId(item)}
                      </span>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}
