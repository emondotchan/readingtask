import { useCallback, useEffect, useMemo, useState } from "react";
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
  ArrowLeftIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  HistoryIcon,
  PencilIcon,
  PlayIcon,
  LockIcon,
} from "lucide-react";
import {
  getTaskDailyTasks,
  getTaskResults,
  previewMonthlyTaskPlan,
  saveDailyTask,
} from "@/api/commands";
import type {
  DailyTask,
  MonthlyTask,
  MonthlyTaskPlanPreview,
  TaskItemResult,
} from "@/types";
import { cn } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { MonthlyTaskRunState } from "@/features/useMonthlyRunner";

interface Props {
  task: MonthlyTask | null;
  onBack: () => void;
  currentRun?: MonthlyTaskRunState;
}

const TASK_TYPE_BADGE_CLASSNAME = {
  Avene:
    "border-primary/20 bg-primary/10 text-primary dark:border-primary/30 dark:bg-primary/15 dark:text-primary",
  Klorane:
    "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300",
} as const;

export function TaskStatusDialog({ task, onBack, currentRun }: Props) {
  const [historyResults, setHistoryResults] = useState<TaskItemResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedDate, setSelectedDate] = useState("all");
  const [currentPage, setCurrentPage] = useState(1);
  const [dailyPlans, setDailyPlans] = useState<DailyTask[]>([]);
  const [dailyPlansLoading, setDailyPlansLoading] = useState(false);
  const [dailyPlansExpanded, setDailyPlansExpanded] = useState(false);
  const [editingDay, setEditingDay] = useState<string | null>(null);
  const [editorText, setEditorText] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [savingDay, setSavingDay] = useState<string | null>(null);
  const pageSize = 10;

  const getResultSucceeded = (item: TaskItemResult) =>
    item.submit_err === 0
      ? true
      : typeof item.submit_err === "number"
        ? false
        : item.outcome === "Success";
  const getResultDate = (item: TaskItemResult) =>
    item.executed_date ?? (currentRun?.date ? `${currentRun.date} 00:00:00` : null);
  const getResultDay = (item: TaskItemResult) => {
    const raw = getResultDate(item);
    return raw ? raw.slice(0, 10) : null;
  };
  const getResultDateDisplay = (item: TaskItemResult) => {
    const raw = getResultDate(item);
    if (!raw) {
      return "—";
    }
    return raw.length === 10 ? `${raw} 00:00:00` : raw;
  };
  const getResultRtnMsg = (item: TaskItemResult) =>
    item.rtn_msg ?? item.response_text ?? "—";
  const getResultReadId = (item: TaskItemResult) => item.read_id ?? "None";

  const loadHistoryResults = useCallback(async () => {
    if (!task) {
      setHistoryResults([]);
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      setHistoryResults(await getTaskResults(task.id));
    } catch (error) {
      console.error(error);
      setHistoryResults([]);
    } finally {
      setLoading(false);
    }
  }, [task]);

  const loadDailyPlans = useCallback(async () => {
    if (!task) {
      setDailyPlans([]);
      setDailyPlansLoading(false);
      return;
    }

    setDailyPlansLoading(true);
    try {
      const progress = await getTaskDailyTasks(task.id);
      if (progress.length > 0) {
        setDailyPlans(progress);
        return;
      }

      const preview: MonthlyTaskPlanPreview = await previewMonthlyTaskPlan(task);
      setDailyPlans(preview.daily_plans);
    } catch (error) {
      console.error(error);
      setDailyPlans([]);
    } finally {
      setDailyPlansLoading(false);
    }
  }, [task]);

  useEffect(() => {
    void loadHistoryResults();
  }, [loadHistoryResults, currentRun?.runState]);

  useEffect(() => {
    void loadDailyPlans();
  }, [loadDailyPlans, currentRun?.runState]);

  useEffect(() => {
    setSelectedDate("all");
    setCurrentPage(1);
    setDailyPlansExpanded(false);
    setEditingDay(null);
    setEditorText("");
    setSaveError(null);
    setSavingDay(null);
  }, [task?.id]);

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
            .map((item) => getResultDay(item))
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
      (item) => getResultDay(item) === selectedDate,
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
          .map((item) => getResultDay(item))
          .filter((value): value is string => Boolean(value)),
      ).size,
    [mergedHistoryResults],
  );

  const failedShopcodesByDate = useMemo(() => {
    const entries = new Map<string, Set<string>>();
    for (const item of mergedHistoryResults) {
      if (getResultSucceeded(item)) {
        continue;
      }

      const day = getResultDay(item);
      if (!day) {
        continue;
      }

      const shopcodes = entries.get(day) ?? new Set<string>();
      shopcodes.add(item.shop_code);
      entries.set(day, shopcodes);
    }

    return entries;
  }, [mergedHistoryResults]);

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
    <div className="fixed inset-0 z-50 overflow-y-auto bg-background">
      <div className="mx-auto flex min-h-full w-full max-w-7xl flex-col gap-6 px-4 py-6 md:px-6 lg:px-8">
        <Card className="border-border shadow-sm">
          <CardHeader className="border-b border-border pb-4">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div className="flex flex-col gap-4 sm:flex-row sm:items-start">
                <Button
                  variant="outline"
                  size="sm"
                  className="w-fit"
                  onClick={onBack}
                >
                  <ArrowLeftIcon className="mr-1 size-3.5" />
                  返回列表
                </Button>
                <div className="flex items-center gap-3">
                  <div className="rounded-lg bg-muted p-2.5 text-muted-foreground">
                    <ActivityIcon className="size-5" />
                  </div>
                  <div className="space-y-1">
                    <CardTitle className="flex flex-wrap items-center gap-2 text-2xl">
                      <span>{task.fc_name}</span>
                      <Badge
                        variant="outline"
                        className={TASK_TYPE_BADGE_CLASSNAME[task.task_type]}
                      >
                        {task.task_type}
                      </Badge>
                    </CardTitle>
                    <p className="font-mono text-xs text-muted-foreground">
                      taskId: {task.id} • courseId: {task.s_course_id} • 经理ID:{" "}
                      {task.s_manager_id}
                    </p>
                  </div>
                </div>
              </div>
              {isRunning && (
                <Badge
                  variant="outline"
                  className="w-fit animate-pulse border-sky-200 bg-sky-50 py-1 px-3 text-sky-700 dark:border-sky-900/60 dark:bg-sky-950/20 dark:text-sky-300"
                >
                  正在执行中...
                </Badge>
              )}
            </div>
          </CardHeader>

          <CardContent className="space-y-8 py-6">
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

            <div className="grid grid-cols-1 gap-4 md:grid-cols-4">
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
                      mergedHistoryResults.filter((item) => getResultSucceeded(item))
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

            <div className="space-y-4">
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                <h3 className="text-sm font-semibold text-foreground">
                  每日任务明细
                </h3>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    setDailyPlansExpanded((previous) => {
                      const next = !previous;
                      if (!next) {
                        setEditingDay(null);
                        setSaveError(null);
                      }
                      return next;
                    });
                  }}
                >
                  {dailyPlansExpanded ? (
                    <ChevronUpIcon className="mr-1 size-3.5" />
                  ) : (
                    <ChevronDownIcon className="mr-1 size-3.5" />
                  )}
                  {dailyPlansExpanded ? "收起明细" : "展开明细"}
                </Button>
              </div>

              {!dailyPlansExpanded ? null : dailyPlansLoading ? (
                <div className="flex items-center justify-center rounded-xl border border-border bg-card p-10">
                  <Spinner />
                </div>
              ) : dailyPlans.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border bg-card/50 p-6 text-sm text-muted-foreground">
                  暂无可展示的每日任务计划。
                </div>
              ) : (
                <div className="grid gap-4">
                  {dailyPlans.map((day) => {
                    const failedShopcodes = failedShopcodesByDate.get(day.date) ?? new Set<string>();
                    const isEditing = editingDay === day.date;
                    const isLocked = day.is_locked;

                    return (
                      <div
                        key={day.date}
                        className="rounded-2xl border border-border bg-card p-4 shadow-sm"
                      >
                        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                          <div className="space-y-3">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="rounded-md bg-muted px-2 py-1 font-mono text-sm text-foreground">
                                {day.date}
                              </span>
                              <Badge variant="secondary">
                                目标 {day.target_count} 家
                              </Badge>
                              <Badge variant="outline">
                                已完成 {day.completed_count}/{day.target_count}
                              </Badge>
                              {isLocked ? (
                                <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300">
                                  <LockIcon className="mr-1 size-3" />
                                  已执行
                                </Badge>
                              ) : (
                                <Badge variant="outline">可编辑</Badge>
                              )}
                              {failedShopcodes.size > 0 && (
                                <Badge className="border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/60 dark:bg-rose-950/20 dark:text-rose-300">
                                  <AlertCircleIcon className="mr-1 size-3" />
                                  错误 {failedShopcodes.size}
                                </Badge>
                              )}
                            </div>

                            <div className="flex flex-wrap gap-2">
                              {day.shopcodes.length === 0 ? (
                                <span className="text-sm text-muted-foreground">
                                  暂无 shopcode
                                </span>
                              ) : (
                                day.shopcodes.map((shopcode) => (
                                  <span
                                    key={`${day.date}:${shopcode}`}
                                    className={cn(
                                      "inline-flex rounded-full border px-2.5 py-1 font-mono text-xs",
                                      failedShopcodes.has(shopcode)
                                        ? "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/60 dark:bg-rose-950/20 dark:text-rose-300"
                                        : "border-border bg-muted/50 text-muted-foreground",
                                    )}
                                  >
                                    {shopcode}
                                  </span>
                                ))
                              )}
                            </div>
                          </div>

                          <div className="flex items-center gap-2">
                            {isLocked ? (
                              <Button size="sm" variant="outline" disabled>
                                已完成，不可编辑
                              </Button>
                            ) : (
                              <Button
                                size="sm"
                                variant={isEditing ? "secondary" : "outline"}
                                onClick={() => {
                                  setEditingDay(day.date);
                                  setEditorText(day.shopcodes.join("\n"));
                                  setSaveError(null);
                                }}
                              >
                                <PencilIcon className="mr-1 size-3.5" />
                                编辑
                              </Button>
                            )}
                          </div>
                        </div>

                        {isEditing && !isLocked && (
                          <div className="mt-4 space-y-3 rounded-xl border border-border bg-muted/20 p-4">
                            <div className="text-sm font-semibold">
                              编辑 {day.date} 的 shopcodes（每行一个）
                            </div>
                            <Textarea
                              value={editorText}
                              onChange={(event) => setEditorText(event.target.value)}
                              className="min-h-40 font-mono"
                            />
                            {saveError && (
                              <p className="text-sm text-rose-600 dark:text-rose-300">
                                {saveError}
                              </p>
                            )}
                            <div className="flex items-center gap-2">
                              <Button
                                onClick={async () => {
                                  const newShopcodes = editorText
                                    .split(/\r?\n/)
                                    .map((value) => value.trim())
                                    .filter(Boolean);

                                  setSavingDay(day.date);
                                  setSaveError(null);

                                  try {
                                    await saveDailyTask({
                                      ...day,
                                      shopcodes: newShopcodes,
                                    });
                                    await loadDailyPlans();
                                    setEditingDay(null);
                                  } catch (error) {
                                    setSaveError(
                                      error instanceof Error
                                        ? error.message
                                        : String(error),
                                    );
                                  } finally {
                                    setSavingDay(null);
                                  }
                                }}
                                disabled={savingDay === day.date}
                              >
                                保存
                              </Button>
                              <Button
                                variant="outline"
                                onClick={() => {
                                  setEditingDay(null);
                                  setSaveError(null);
                                }}
                              >
                                取消
                              </Button>
                            </div>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

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
                        <TableHead className="w-16 text-center text-[11px] font-bold uppercase tracking-wider">
                          状态
                        </TableHead>
                        <TableHead className="w-32 text-left text-[11px] font-bold uppercase tracking-wider">
                          执行日期
                        </TableHead>
                        <TableHead className="w-64 text-left text-[11px] font-bold uppercase tracking-wider">
                          OpenID
                        </TableHead>
                        <TableHead className="w-24 text-left text-[11px] font-bold uppercase tracking-wider">
                          门店代码
                        </TableHead>
                        <TableHead className="text-left text-[11px] font-bold uppercase tracking-wider">
                          RtnMsg
                        </TableHead>
                        <TableHead className="w-32 text-left text-[11px] font-bold uppercase tracking-wider">
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
                              <TableCell className="py-3 text-center">
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
                                {getResultDateDisplay(item)}
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
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
