import { useEffect, useMemo, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  CheckCircle2Icon,
  AlertCircleIcon,
  ActivityIcon,
  HistoryIcon,
  PlayIcon,
} from "lucide-react";
import { getTaskResults } from "@/api/commands";
import type { TaskItemResult, MonthlyTask } from "@/types";
import { cn } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { MonthlyTaskRunState } from "@/features/useMonthlyRunner";

interface Props {
  task: MonthlyTask | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  currentRun?: MonthlyTaskRunState;
}

export function TaskStatusDialog({
  task,
  open,
  onOpenChange,
  currentRun,
}: Props) {
  const [historyResults, setHistoryResults] = useState<TaskItemResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedDate, setSelectedDate] = useState("all");
  const [currentPage, setCurrentPage] = useState(1);
  const pageSize = 10;

  const getResultSucceeded = (item: TaskItemResult) =>
    item.submit_err === 0
      ? true
      : typeof item.submit_err === "number"
        ? false
        : item.outcome === "Success";
  const getResultDate = (item: TaskItemResult) =>
    item.executed_date ?? currentRun?.date ?? null;
  const getResultRtnMsg = (item: TaskItemResult) =>
    item.rtn_msg ?? item.error_message ?? item.response_text ?? "—";
  const getResultReadId = (item: TaskItemResult) => item.read_id ?? "None";

  useEffect(() => {
    if (open && task) {
      setLoading(true);
      getTaskResults(task.id)
        .then(setHistoryResults)
        .catch(console.error)
        .finally(() => setLoading(false));
    }
  }, [open, task, currentRun?.runState]);

  useEffect(() => {
    if (!open) {
      return;
    }

    setSelectedDate("all");
    setCurrentPage(1);
  }, [open, task?.id]);

  const isRunning = currentRun?.runState === "running";
  const mergedHistoryResults = useMemo(() => {
    if (!isRunning || !currentRun) {
      return historyResults;
    }

    const seen = new Set<string>();
    return [...currentRun.items]
      .reverse()
      .concat(historyResults)
      .filter((item) => {
        const key = [
          getResultDate(item) ?? "unknown-date",
          item.index,
          item.open_id,
          item.shop_code,
          item.outcome,
          item.http_status ?? "ERR",
        ].join(":");

        if (seen.has(key)) {
          return false;
        }

        seen.add(key);
        return true;
      });
  }, [currentRun, historyResults, isRunning]);
  const dateOptions = useMemo(
    () =>
      Array.from(
        new Set(
          mergedHistoryResults
            .map((item) => getResultDate(item))
            .filter((value): value is string => Boolean(value)),
        ),
      ).sort((a, b) => b.localeCompare(a)),
    [mergedHistoryResults],
  );
  const filteredHistoryResults = useMemo(() => {
    if (selectedDate === "all") {
      return mergedHistoryResults;
    }

    return mergedHistoryResults.filter(
      (item) => getResultDate(item) === selectedDate,
    );
  }, [mergedHistoryResults, selectedDate]);
  const totalPages = Math.max(
    1,
    Math.ceil(filteredHistoryResults.length / pageSize),
  );
  const paginatedResults = useMemo(() => {
    const start = (currentPage - 1) * pageSize;
    return filteredHistoryResults.slice(start, start + pageSize);
  }, [currentPage, filteredHistoryResults]);
  const completedDays = useMemo(
    () =>
      new Set(
        mergedHistoryResults
          .filter((item) => getResultSucceeded(item))
          .map((item) => getResultDate(item))
          .filter((value): value is string => Boolean(value)),
      ).size,
    [mergedHistoryResults],
  );

  useEffect(() => {
    setCurrentPage(1);
  }, [selectedDate]);

  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [currentPage, totalPages]);

  const progressValue =
    currentRun && currentRun.requestedCount > 0
      ? (currentRun.processedCount / currentRun.requestedCount) * 100
      : 0;

  if (!task) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-5xl lg:max-w-6xl w-[95vw] sm:w-[90vw] overflow-hidden flex flex-col max-h-[90vh]">
        <DialogHeader className="shrink-0 border-b pb-4">
          <div className="flex items-center justify-between pr-2">
            <div className="flex items-center gap-3">
              <div className="rounded-lg bg-muted p-2.5 text-muted-foreground">
                <ActivityIcon className="size-5" />
              </div>
              <div>
                <DialogTitle className="text-xl">
                  任务详情: {task.fc_name}
                </DialogTitle>
                <p className="mt-0.5 font-mono text-xs uppercase text-muted-foreground">
                  ID: {task.s_course_id} • 经理 ID: {task.s_manager_id}
                </p>
              </div>
            </div>
            {isRunning && (
              <Badge
                variant="outline"
                className="animate-pulse border-sky-200 bg-sky-50 py-1 px-3 mr-3 text-sky-700 dark:border-sky-900/60 dark:bg-sky-950/20 dark:text-sky-300"
              >
                正在执行中...
              </Badge>
            )}
          </div>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto py-6 space-y-8">
          {/* Real-time Progress Section (Only visible when running) */}
          {isRunning && (
            <div className="space-y-4 rounded-xl border border-sky-200/70 bg-sky-500/8 p-6 dark:border-sky-900/60 dark:bg-sky-950/20">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <PlayIcon className="size-4 animate-spin-slow text-sky-600 dark:text-sky-400" />
                  <h3 className="font-semibold text-sky-900 dark:text-sky-100">
                    当前任务进度
                  </h3>
                </div>
                <span className="text-sm font-mono font-bold text-sky-700 dark:text-sky-300">
                  {currentRun.processedCount} / {currentRun.requestedCount}
                </span>
              </div>
              <Progress
                value={progressValue}
                className="h-2 bg-sky-100 dark:bg-sky-950/50"
              />
              {currentRun.items.length > 0 && (
                <div className="flex items-center gap-2 text-xs text-sky-600 dark:text-sky-300">
                  <span className="shrink-0">最新结果:</span>
                  <span className="truncate opacity-80">
                    门店{" "}
                    {currentRun.items[currentRun.items.length - 1].shop_code} —{" "}
                    {getResultSucceeded(
                      currentRun.items[currentRun.items.length - 1],
                    )
                      ? "成功"
                      : "失败"}
                  </span>
                </div>
              )}
            </div>
          )}

          {/* Statistics Grid */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
            <div className="flex h-24 flex-col justify-between rounded-xl border border-border bg-card p-4 shadow-sm">
              <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">
                月度总目标
              </span>
              <div className="flex items-baseline gap-1">
                <span className="text-2xl font-bold text-foreground">
                  {task.total_target}
                </span>
                <span className="text-xs text-muted-foreground">阅读</span>
              </div>
            </div>
            <div className="flex h-24 flex-col justify-between rounded-xl border border-border bg-card p-4 shadow-sm">
              <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">
                完成天数
              </span>
              <div className="flex items-baseline gap-1">
                <span className="text-2xl font-bold text-foreground">
                  {completedDays}
                </span>
                <span className="text-xs text-muted-foreground">
                  / {task.target_days} 天
                </span>
              </div>
            </div>
            <div className="flex h-24 flex-col justify-between rounded-xl border border-emerald-200 bg-emerald-50/20 p-4 shadow-sm dark:border-emerald-900/50 dark:bg-emerald-950/20">
              <span className="text-[10px] font-bold uppercase tracking-widest text-emerald-700/70 dark:text-emerald-300/70">
                累计已完成
              </span>
              <div className="flex items-baseline gap-1">
                <span className="text-2xl font-bold text-emerald-600 dark:text-emerald-300">
                  {
                    mergedHistoryResults.filter((r) => getResultSucceeded(r))
                      .length
                  }
                </span>
                <span className="text-xs text-emerald-600/60 dark:text-emerald-300/70">
                  阅读
                </span>
              </div>
            </div>
            <div className="flex h-24 flex-col justify-between rounded-xl border border-border bg-card p-4 shadow-sm">
              <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">
                任务类型
              </span>
              <div className="flex items-center">
                <Badge
                  variant="outline"
                  className={cn(
                    "px-3 py-1 text-sm font-semibold",
                    task.task_type === "Avene"
                      ? "border-primary/20 bg-primary/10 text-primary dark:border-primary/30 dark:bg-primary/15 dark:text-primary"
                      : "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300",
                  )}
                >
                  {task.task_type}
                </Badge>
              </div>
            </div>
            <div className="flex h-24 flex-col justify-between rounded-xl border border-border bg-card p-4 shadow-sm">
              <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">
                总执行记录
              </span>
              <div className="flex items-baseline gap-1">
                <span className="text-2xl font-bold text-foreground">
                  {mergedHistoryResults.length}
                </span>
                <span className="text-xs text-muted-foreground">条</span>
              </div>
            </div>
          </div>

          {/* History Records Table */}
          <div className="space-y-4">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex items-center gap-2">
                <HistoryIcon className="size-4 text-muted-foreground" />
                <h3 className="text-sm font-semibold text-foreground">
                  执行历史明细
                </h3>
                <span className="text-xs text-muted-foreground">
                  共 {filteredHistoryResults.length} 条
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-muted-foreground">按天筛选</span>
                <Select value={selectedDate} onValueChange={setSelectedDate}>
                  <SelectTrigger className="w-45 bg-background">
                    <SelectValue placeholder="全部日期" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部日期</SelectItem>
                    {dateOptions.map((date) => (
                      <SelectItem key={date} value={date}>
                        {date}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
              <ScrollArea className="h-100">
                <Table>
                  <TableHeader>
                    <TableRow className="bg-muted/60 hover:bg-muted/60">
                      <TableHead className="w-16 text-center text-[11px] uppercase tracking-wider font-bold">
                        状态
                      </TableHead>
                      <TableHead className="w-32 text-left text-[11px] uppercase tracking-wider font-bold">
                        执行日期
                      </TableHead>
                      <TableHead className="w-64 text-left text-[11px] uppercase tracking-wider font-bold">
                        OpenID
                      </TableHead>
                      <TableHead className="w-24 text-left text-[11px] uppercase tracking-wider font-bold">
                        门店代码
                      </TableHead>
                      <TableHead className="text-left text-[11px] uppercase tracking-wider font-bold">
                        RtnMsg
                      </TableHead>
                      <TableHead className="w-32 text-left text-[11px] uppercase tracking-wider font-bold">
                        ReadID
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {loading ? (
                      <TableRow>
                        <TableCell colSpan={6} className="h-48 text-center">
                          <div className="flex flex-col items-center gap-2 text-muted-foreground">
                            <Spinner className="size-5" />
                            <span className="text-xs">
                              正在调取数据库记录...
                            </span>
                          </div>
                        </TableCell>
                      </TableRow>
                    ) : mergedHistoryResults.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={6}
                          className="h-48 text-center text-xs italic text-muted-foreground"
                        >
                          暂无任何执行历史记录
                        </TableCell>
                      </TableRow>
                    ) : filteredHistoryResults.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={6}
                          className="h-48 text-center text-xs italic text-muted-foreground"
                        >
                          当前日期筛选下暂无执行记录
                        </TableCell>
                      </TableRow>
                    ) : (
                      paginatedResults.map((item, idx) => {
                        const isSuccess = getResultSucceeded(item);
                        return (
                          <TableRow
                            key={[
                              getResultDate(item) ?? "unknown-date",
                              item.index,
                              item.open_id,
                              item.shop_code,
                              idx,
                            ].join(":")}
                            className="transition-colors hover:bg-muted/40"
                          >
                            <TableCell className="text-center py-3">
                              {isSuccess ? (
                                <div className="mx-auto flex size-5 items-center justify-center rounded-full bg-emerald-100 text-emerald-600 dark:bg-emerald-950/30 dark:text-emerald-300">
                                  <CheckCircle2Icon className="size-3" />
                                </div>
                              ) : (
                                <div className="mx-auto flex size-5 items-center justify-center rounded-full bg-rose-100 text-rose-600 dark:bg-rose-950/30 dark:text-rose-300">
                                  <AlertCircleIcon className="size-3" />
                                </div>
                              )}
                            </TableCell>
                            <TableCell className="py-3 text-left font-mono text-xs text-muted-foreground">
                              {getResultDate(item) ?? "—"}
                            </TableCell>
                            <TableCell className="py-3 text-left">
                              <code className="block rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                                {item.open_id}
                              </code>
                            </TableCell>
                            <TableCell className="py-3 text-left font-mono text-xs font-medium text-foreground">
                              {item.shop_code}
                            </TableCell>
                            <TableCell className="py-3 text-left align-top">
                              <div
                                className={cn(
                                  "pr-4 text-left text-xs italic transition-all line-clamp-2 group-hover:line-clamp-none whitespace-pre-wrap break-all",
                                  isSuccess
                                    ? "text-muted-foreground"
                                    : "text-rose-600 dark:text-rose-300",
                                )}
                              >
                                {getResultRtnMsg(item)}
                              </div>
                            </TableCell>
                            <TableCell className="py-3 text-left">
                              <code className="block rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                                {getResultReadId(item)}
                              </code>
                            </TableCell>
                          </TableRow>
                        );
                      })
                    )}
                  </TableBody>
                </Table>
              </ScrollArea>
            </div>
            {filteredHistoryResults.length > 0 && (
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-muted-foreground">
                  第 {currentPage} / {totalPages} 页，每页 {pageSize} 条
                </p>
                <div className="flex items-center gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      setCurrentPage((page) => Math.max(1, page - 1))
                    }
                    disabled={currentPage <= 1}
                  >
                    上一页
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      setCurrentPage((page) => Math.min(totalPages, page + 1))
                    }
                    disabled={currentPage >= totalPages}
                  >
                    下一页
                  </Button>
                </div>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
