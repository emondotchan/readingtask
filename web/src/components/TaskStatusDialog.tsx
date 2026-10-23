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
  CheckIcon,
  AlertCircleIcon,
  ActivityIcon,
  ArrowLeftIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  HistoryIcon,
  PencilIcon,
  PlayIcon,
  RotateCcwIcon,
  SlidersIcon,
  LockIcon,
  SquareIcon,
} from "lucide-react";
import {
  getTaskDailyTasks,
  getTaskResults,
  retryTaskResult,
  saveDailyTask,
  updateMonthlyTaskDailyTargetRange,
} from "@/api/commands";
import type { DailyTask, MonthlyTask, MonthlyTaskPlanPreview, TaskItemResult } from "@/types";
import { cn, getErrorMessage } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
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
  executeDaily?: (taskId: string, date: string) => Promise<void>;
  pauseDaily?: (taskId: string) => Promise<void>;
  onTaskUpdated?: (task: MonthlyTask) => void;
}

const TASK_TYPE_BADGE_CLASSNAME = {
  Avene:
    "border-primary/20 bg-primary/10 text-primary dark:border-primary/30 dark:bg-primary/15 dark:text-primary",
  Klorane:
    "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300",
} as const;

function getDailyTaskProgressTotal(task: DailyTask) {
  return task.target_count;
}

function isCompletedDay(task: DailyTask) {
  return task.completed_count >= task.target_count;
}

const DAILY_TASK_STATUS_META = {
  not_started: {
    label: "未开始",
    className: "",
  },
  running: {
    label: "执行中",
    className:
      "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900/60 dark:bg-sky-950/20 dark:text-sky-300",
  },
  paused: {
    label: "暂停",
    className:
      "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900/60 dark:bg-amber-950/20 dark:text-amber-300",
  },
  completed: {
    label: "已完成",
    className:
      "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300",
  },
} as const;

export function TaskStatusDialog({
  task,
  onBack,
  currentRun,
  executeDaily,
  pauseDaily,
  onTaskUpdated,
}: Props) {
  const [currentTask, setCurrentTask] = useState<MonthlyTask | null>(task);
  useEffect(() => {
    setCurrentTask(task);
  }, [task]);

  const activeTask = currentTask ?? task;

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
  const [retryingResultKey, setRetryingResultKey] = useState<string | null>(null);
  const [retryMessage, setRetryMessage] = useState<string | null>(null);
  const [batchRetryProgress, setBatchRetryProgress] = useState<{
    completed: number;
    total: number;
  } | null>(null);
  const [isEditingRange, setIsEditingRange] = useState(false);
  const [minTargetInput, setMinTargetInput] = useState("");
  const [maxTargetInput, setMaxTargetInput] = useState("");
  const [rangeSaving, setRangeSaving] = useState(false);
  const [rangeError, setRangeError] = useState<string | null>(null);
  const [rangeSuccessMessage, setRangeSuccessMessage] = useState<string | null>(null);
  const pageSize = 10;

  const getResultSucceeded = useCallback(
    (item: TaskItemResult) =>
      item.submit_err === 0
        ? true
        : typeof item.submit_err === "number"
          ? false
          : item.outcome === "Success",
    [],
  );
  const getResultDate = useCallback(
    (item: TaskItemResult) =>
      item.executed_date ?? (currentRun?.date ? `${currentRun.date} 00:00:00` : null),
    [currentRun?.date],
  );
  const getResultDay = useCallback(
    (item: TaskItemResult) => {
      const raw = getResultDate(item);
      return raw ? raw.slice(0, 10) : null;
    },
    [getResultDate],
  );
  const getResultDateDisplay = useCallback(
    (item: TaskItemResult) => {
      const raw = getResultDate(item);
      if (!raw) {
        return "—";
      }
      return raw.length === 10 ? `${raw} 00:00:00` : raw;
    },
    [getResultDate],
  );
  const getResultRtnMsg = (item: TaskItemResult) => item.rtn_msg ?? item.response_text ?? "—";
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

  const getResultKey = useCallback(
    (item: TaskItemResult) =>
      item.result_id?.toString() ??
      [getResultDate(item) ?? "unknown-date", item.index, item.open_id, item.shop_code].join(":"),
    [getResultDate],
  );

  const applyRetriedResult = useCallback((retried: TaskItemResult) => {
    setHistoryResults((previous) => [
      retried,
      ...previous.filter((result) => result.result_id !== retried.result_id),
    ]);
  }, []);

  const handleRetryResult = useCallback(
    async (item: TaskItemResult) => {
      if (!task || item.result_id == null) return;

      const resultKey = getResultKey(item);
      setRetryingResultKey(resultKey);
      setRetryMessage(null);
      try {
        const retried = await retryTaskResult(task.id, item.result_id);
        applyRetriedResult(retried);
        setCurrentPage(1);
        setRetryMessage(
          getResultSucceeded(retried)
            ? `门店 ${item.shop_code} 重做成功`
            : `门店 ${item.shop_code} 重做后仍失败`,
        );
      } catch (error) {
        setRetryMessage(`门店 ${item.shop_code} 重做失败：${getErrorMessage(error)}`);
      } finally {
        setRetryingResultKey(null);
      }
    },
    [applyRetriedResult, getResultKey, getResultSucceeded, task],
  );

  const loadDailyPlans = useCallback(async () => {
    if (!task) {
      setDailyPlans([]);
      setDailyPlansLoading(false);
      return;
    }

    setDailyPlansLoading(true);
    try {
      const progress = await getTaskDailyTasks(task.id);
      setDailyPlans(progress);
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
    setRetryingResultKey(null);
    setRetryMessage(null);
    setBatchRetryProgress(null);
    setIsEditingRange(false);
    setMinTargetInput("");
    setMaxTargetInput("");
    setRangeError(null);
    setRangeSuccessMessage(null);
  }, [task?.id]);

  const hasCustomRange =
    activeTask?.min_daily_target != null && activeTask?.max_daily_target != null;

  const handleStartEditRange = () => {
    setMinTargetInput(activeTask?.min_daily_target?.toString() ?? "");
    setMaxTargetInput(activeTask?.max_daily_target?.toString() ?? "");
    setRangeError(null);
    setRangeSuccessMessage(null);
    setIsEditingRange(true);
  };

  const handleSaveRange = async () => {
    if (!activeTask) return;

    const trimmedMin = minTargetInput.trim();
    const trimmedMax = maxTargetInput.trim();

    if (trimmedMin === "" && trimmedMax === "") {
      await handleResetRange();
      return;
    }

    if (trimmedMin === "" || trimmedMax === "") {
      setRangeError("请同时设置最小与最大数量，或清空输入以恢复默认 15~25 家");
      return;
    }

    const minNum = Number(trimmedMin);
    const maxNum = Number(trimmedMax);

    if (!Number.isInteger(minNum) || !Number.isInteger(maxNum) || minNum < 1 || maxNum < 1) {
      setRangeError("门店数量必须为大于 0 的有效整数");
      return;
    }

    if (minNum > maxNum) {
      setRangeError("最小数量不能大于最大数量");
      return;
    }

    setRangeSaving(true);
    setRangeError(null);
    setRangeSuccessMessage(null);

    try {
      const updated = await updateMonthlyTaskDailyTargetRange(activeTask.id, minNum, maxNum);
      setCurrentTask(updated);
      onTaskUpdated?.(updated);
      setIsEditingRange(false);
      setRangeSuccessMessage(
        minNum === maxNum
          ? `已单独设置每日预期完成门店数为 ${minNum} 家（覆盖默认 15~25 家）`
          : `已单独设置每日预期完成门店数为 ${minNum}~${maxNum} 家（覆盖默认 15~25 家）`,
      );
    } catch (error) {
      setRangeError(`保存失败：${getErrorMessage(error)}`);
    } finally {
      setRangeSaving(false);
    }
  };

  const handleResetRange = async () => {
    if (!activeTask) return;

    setRangeSaving(true);
    setRangeError(null);
    setRangeSuccessMessage(null);

    try {
      const updated = await updateMonthlyTaskDailyTargetRange(activeTask.id, null, null);
      setCurrentTask(updated);
      onTaskUpdated?.(updated);
      setMinTargetInput("");
      setMaxTargetInput("");
      setIsEditingRange(false);
      setRangeSuccessMessage("已恢复为默认每日预期完成门店数（15~25 家）");
    } catch (error) {
      setRangeError(`重置失败：${getErrorMessage(error)}`);
    } finally {
      setRangeSaving(false);
    }
  };

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
  }, [currentRun, getResultDate, historyResults, isRunning]);

  const dateOptions = useMemo(
    () =>
      Array.from(
        new Set(
          mergedHistoryResults
            .map((item) => getResultDay(item))
            .filter((value): value is string => Boolean(value)),
        ),
      ).sort((a, b) => b.localeCompare(a)),
    [getResultDay, mergedHistoryResults],
  );

  const filteredHistoryResults = useMemo(() => {
    if (selectedDate === "all") {
      return mergedHistoryResults;
    }

    return mergedHistoryResults.filter((item) => getResultDay(item) === selectedDate);
  }, [getResultDay, mergedHistoryResults, selectedDate]);

  const retryableFilteredResults = useMemo(
    () =>
      filteredHistoryResults.filter((item) => item.result_id != null && !getResultSucceeded(item)),
    [filteredHistoryResults, getResultSucceeded],
  );

  const handleBatchRetry = useCallback(async () => {
    if (!task || isRunning || retryableFilteredResults.length === 0) return;

    const total = retryableFilteredResults.length;
    let successCount = 0;
    let failedCount = 0;
    let requestErrorCount = 0;
    setBatchRetryProgress({ completed: 0, total });
    setRetryMessage(null);

    for (const [index, item] of retryableFilteredResults.entries()) {
      if (item.result_id == null) continue;

      setRetryingResultKey(getResultKey(item));
      try {
        const retried = await retryTaskResult(task.id, item.result_id);
        applyRetriedResult(retried);
        if (getResultSucceeded(retried)) {
          successCount += 1;
        } else {
          failedCount += 1;
        }
      } catch (error) {
        console.error(error);
        requestErrorCount += 1;
      } finally {
        setBatchRetryProgress({ completed: index + 1, total });
      }
    }

    setCurrentPage(1);
    setRetryingResultKey(null);
    setBatchRetryProgress(null);
    setRetryMessage(
      `批量重做完成：成功 ${successCount}，仍失败 ${failedCount}，请求异常 ${requestErrorCount}`,
    );
  }, [
    applyRetriedResult,
    getResultKey,
    getResultSucceeded,
    isRunning,
    retryableFilteredResults,
    task,
  ]);

  const totalPages = Math.max(1, Math.ceil(filteredHistoryResults.length / pageSize));

  const paginatedResults = useMemo(() => {
    const start = (currentPage - 1) * pageSize;
    return filteredHistoryResults.slice(start, start + pageSize);
  }, [currentPage, filteredHistoryResults]);

  const completedCount = useMemo(
    () => dailyPlans.reduce((sum, item) => sum + item.completed_count, 0),
    [dailyPlans],
  );

  const shopReadingStatusMap = useMemo(() => {
    const entries = new Map<string, "success" | "failed">();
    for (const item of mergedHistoryResults) {
      if (entries.has(item.shop_code)) {
        continue;
      }
      const hasReadId = Boolean(item.read_id?.trim());
      entries.set(item.shop_code, getResultSucceeded(item) && hasReadId ? "success" : "failed");
    }

    return entries;
  }, [getResultSucceeded, mergedHistoryResults]);

  const usedShopCount = useMemo(() => {
    const seen = new Set<string>();
    for (const item of mergedHistoryResults) {
      if (item.shop_code && getResultSucceeded(item)) {
        seen.add(item.shop_code);
      }
    }
    return seen.size;
  }, [getResultSucceeded, mergedHistoryResults]);

  const firstPendingDay = useMemo(
    () => dailyPlans.find((d) => !isCompletedDay(d) && !d.is_locked),
    [dailyPlans],
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

  if (!activeTask) return null;

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto bg-background">
      <div className="mx-auto flex min-h-full w-full max-w-7xl flex-col gap-6 px-4 py-6 md:px-6 lg:px-8">
        <Card className="border-border shadow-sm">
          <CardHeader className="border-b border-border pb-4">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div className="flex flex-col gap-4 sm:flex-row sm:items-start">
                <Button variant="outline" size="sm" className="w-fit" onClick={onBack}>
                  <ArrowLeftIcon className="mr-1 size-3.5" />
                  返回列表
                </Button>
                <div className="flex items-center gap-3">
                  <div className="rounded-lg bg-muted p-2.5 text-muted-foreground">
                    <ActivityIcon className="size-5" />
                  </div>
                  <div className="space-y-1">
                    <CardTitle className="flex flex-wrap items-center gap-2 text-2xl">
                      <span>{activeTask.fc_name}</span>
                      <Badge
                        variant="outline"
                        className={TASK_TYPE_BADGE_CLASSNAME[activeTask.task_type]}
                      >
                        {activeTask.task_type}
                      </Badge>
                    </CardTitle>
                    <p className="font-mono text-xs text-muted-foreground">
                      taskId: {activeTask.id} • courseId: {activeTask.s_course_id} • UID: {activeTask.s_manager_id}
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
                    <h3 className="font-semibold text-sky-900 dark:text-sky-100">当前任务进度</h3>
                  </div>
                  <span className="text-sm font-mono font-bold text-sky-700 dark:text-sky-300">
                    {currentRun.processedCount} / {currentRun.requestedCount}
                  </span>
                </div>
                <Progress value={progressValue} className="h-2 bg-sky-100 dark:bg-sky-950/50" />
                {currentRun.items.length > 0 && (
                  <div className="flex items-center gap-2 text-xs text-sky-600 dark:text-sky-300">
                    <span className="shrink-0">最新结果:</span>
                    <span className="truncate opacity-80">
                      门店 {currentRun.items[currentRun.items.length - 1].shop_code} —{" "}
                      {getResultSucceeded(currentRun.items[currentRun.items.length - 1])
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
                  月度目标
                </span>
                <div className="flex items-baseline gap-1">
                  <span className="text-2xl font-bold text-foreground">
                    {activeTask.total_target}
                  </span>
                  <span className="text-xs text-muted-foreground">门店</span>
                </div>
              </div>
              <div className="flex h-24 flex-col justify-between rounded-xl border border-border bg-card p-4 shadow-sm">
                <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">
                  已使用门店
                </span>
                <div className="flex items-baseline gap-1">
                  <span className="text-2xl font-bold text-foreground">{usedShopCount}</span>
                  <span className="text-xs text-muted-foreground">/ {activeTask.total_target} 家</span>
                </div>
              </div>
              <div className="flex h-24 flex-col justify-between rounded-xl border border-emerald-200 bg-emerald-50/20 p-4 shadow-sm dark:border-emerald-900/50 dark:bg-emerald-950/20">
                <span className="text-[10px] font-bold uppercase tracking-widest text-emerald-700/70 dark:text-emerald-300/70">
                  累计成功
                </span>
                <div className="flex items-baseline gap-1">
                  <span className="text-2xl font-bold text-emerald-600 dark:text-emerald-300">
                    {completedCount}
                  </span>
                  <span className="text-xs text-emerald-600/60 dark:text-emerald-300/70">阅读</span>
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

            {/* 每日预期完成门店数量范围设置 */}
            <div className="rounded-xl border border-border bg-card p-4 sm:p-5 shadow-sm transition-all">
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div className="space-y-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <SlidersIcon className="size-4 text-primary" />
                    <span className="text-sm font-semibold text-foreground">
                      每日预期完成门店数量范围
                    </span>
                    {hasCustomRange ? (
                      <Badge
                        variant="outline"
                        className="border-primary/30 bg-primary/10 text-primary dark:border-primary/40 dark:bg-primary/15"
                      >
                        已单独设置:{" "}
                        {activeTask.min_daily_target === activeTask.max_daily_target
                          ? `${activeTask.min_daily_target} 家/天`
                          : `${activeTask.min_daily_target} ~ ${activeTask.max_daily_target} 家/天`}
                      </Badge>
                    ) : (
                      <Badge variant="outline" className="text-muted-foreground">
                        默认: 15 ~ 25 家/天
                      </Badge>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {hasCustomRange
                      ? "已单独设置每日目标门店数范围（覆盖默认 15~25 家），后续自动生成每日任务时将在此范围内分配。"
                      : "未单独设置范围时，生成每日任务将默认按 15~25 家门店分配。"}
                  </p>
                </div>

                {!isEditingRange && (
                  <div className="flex items-center gap-2 shrink-0">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleStartEditRange}
                    >
                      <PencilIcon className="mr-1.5 size-3.5" />
                      {hasCustomRange ? "修改范围" : "设置范围"}
                    </Button>
                  </div>
                )}
              </div>

              {isEditingRange && (
                <div className="mt-4 pt-3.5 border-t border-border/70 flex flex-col gap-3">
                  <div className="flex flex-wrap items-center gap-3">
                    <span className="text-xs font-medium text-foreground shrink-0">
                      期望每日门店范围：
                    </span>
                    <div className="flex items-center gap-2">
                      <Input
                        type="number"
                        min={1}
                        placeholder="15"
                        className="w-20 h-8 text-center text-sm font-mono"
                        value={minTargetInput}
                        onChange={(e) => {
                          setMinTargetInput(e.target.value);
                          setRangeError(null);
                        }}
                      />
                      <span className="text-sm text-muted-foreground font-medium">~</span>
                      <Input
                        type="number"
                        min={1}
                        placeholder="25"
                        className="w-20 h-8 text-center text-sm font-mono"
                        value={maxTargetInput}
                        onChange={(e) => {
                          setMaxTargetInput(e.target.value);
                          setRangeError(null);
                        }}
                      />
                      <span className="text-xs text-muted-foreground">家 / 天</span>
                    </div>

                    <div className="flex items-center gap-2 ml-auto">
                      <Button
                        variant="default"
                        size="sm"
                        disabled={rangeSaving}
                        onClick={handleSaveRange}
                      >
                        {rangeSaving ? (
                          <Spinner className="mr-1.5 size-3.5" />
                        ) : (
                          <CheckIcon className="mr-1.5 size-3.5" />
                        )}
                        保存
                      </Button>
                      {hasCustomRange && (
                        <Button
                          variant="secondary"
                          size="sm"
                          disabled={rangeSaving}
                          onClick={handleResetRange}
                          title="恢复系统默认 15~25 家"
                        >
                          <RotateCcwIcon className="mr-1.5 size-3.5" />
                          恢复默认
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={rangeSaving}
                        onClick={() => {
                          setIsEditingRange(false);
                          setRangeError(null);
                        }}
                      >
                        取消
                      </Button>
                    </div>
                  </div>

                  <p className="text-[11px] text-muted-foreground">
                    提示：单独设置后将覆盖默认的 15~25 家；如需恢复默认，可点击“恢复默认”或清空输入后保存。
                  </p>

                  {rangeError && (
                    <div className="flex items-center gap-1.5 text-xs text-rose-500 dark:text-rose-400 font-medium">
                      <AlertCircleIcon className="size-3.5 shrink-0" />
                      <span>{rangeError}</span>
                    </div>
                  )}
                </div>
              )}

              {rangeSuccessMessage && !isEditingRange && (
                <div className="mt-3 pt-2.5 border-t border-border/50 flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400 font-medium">
                  <CheckCircle2Icon className="size-3.5 shrink-0" />
                  <span>{rangeSuccessMessage}</span>
                </div>
              )}
            </div>

            <div className="space-y-4">
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-semibold text-foreground">每日任务明细</h3>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  {firstPendingDay && !isRunning && (
                    <Button
                      size="sm"
                      variant="default"
                      className="shadow-sm"
                      onClick={() => {
                        if (!dailyPlansExpanded) {
                          setDailyPlansExpanded(true);
                        }
                        void executeDaily?.(activeTask.id, firstPendingDay.date);
                      }}
                    >
                      <PlayIcon className="mr-1 size-3.5" />
                      执行待办 ({firstPendingDay.date})
                    </Button>
                  )}
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
                    const successfulShopCount = day.shopcodes.filter(
                      (shopcode) => shopReadingStatusMap.get(shopcode) === "success",
                    ).length;
                    const failedShopCount = day.shopcodes.filter(
                      (shopcode) => shopReadingStatusMap.get(shopcode) === "failed",
                    ).length;
                    const isEditing = editingDay === day.date;
                    const isLocked = day.is_locked;
                    const progressTotal = getDailyTaskProgressTotal(day);
                    const statusMeta = DAILY_TASK_STATUS_META[day.run_status];
                    const isRunningDay =
                      currentRun?.runState === "running" && currentRun?.date === day.date;
                    const isPausedDay =
                      currentRun?.date === day.date
                        ? currentRun?.runState === "paused"
                        : day.run_status === "paused";

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
                              <Badge variant="secondary">目标 {progressTotal} 家</Badge>
                              <Badge variant="outline">
                                已完成 {day.completed_count}/{progressTotal}
                              </Badge>
                              <Badge variant="outline" className={statusMeta.className}>
                                {statusMeta.label}
                              </Badge>
                              {isLocked ? (
                                <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300">
                                  <LockIcon className="mr-1 size-3" />
                                  已执行
                                </Badge>
                              ) : (
                                <Badge variant="outline">可编辑</Badge>
                              )}
                              {successfulShopCount > 0 && (
                                <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300">
                                  <CheckCircle2Icon className="mr-1 size-3" />
                                  成功 {successfulShopCount}
                                </Badge>
                              )}
                              {failedShopCount > 0 && (
                                <Badge className="border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/60 dark:bg-rose-950/20 dark:text-rose-300">
                                  <AlertCircleIcon className="mr-1 size-3" />
                                  失败 {failedShopCount}
                                </Badge>
                              )}
                            </div>

                            <div className="flex flex-wrap gap-2">
                              {day.shopcodes.length === 0 ? (
                                <span className="text-sm text-muted-foreground">暂无 shopcode</span>
                              ) : (
                                day.shopcodes.map((shopcode) => {
                                  const readingStatus = shopReadingStatusMap.get(shopcode);
                                  return (
                                    <Badge
                                      key={`${day.date}:${shopcode}`}
                                      variant="outline"
                                      className={cn(
                                        "font-mono",
                                        readingStatus === "success"
                                          ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300"
                                          : readingStatus === "failed"
                                            ? "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/60 dark:bg-rose-950/20 dark:text-rose-300"
                                            : "border-border bg-muted/50 text-muted-foreground",
                                      )}
                                    >
                                      {shopcode}
                                    </Badge>
                                  );
                                })
                              )}
                            </div>
                          </div>

                          <div className="flex items-center gap-2">
                            {isLocked ? (
                              <Button size="sm" variant="outline" disabled>
                                已完成，不可编辑
                              </Button>
                            ) : (
                              <>
                                <Button
                                  size="sm"
                                  variant="default"
                                  className="shadow-sm"
                                  disabled={
                                    currentRun !== undefined &&
                                    currentRun.runState === "running" &&
                                    currentRun.date !== day.date
                                  }
                                  onClick={() => {
                                    if (isRunningDay) {
                                      void pauseDaily?.(activeTask.id);
                                      return;
                                    }
                                    void executeDaily?.(activeTask.id, day.date);
                                  }}
                                >
                                  {isRunningDay ? (
                                    <>
                                      <SquareIcon className="mr-1 size-3.5" />
                                      暂停
                                    </>
                                  ) : (
                                    <>
                                      <PlayIcon className="mr-1 size-3.5" />
                                      {isPausedDay ? "继续" : "执行"}
                                    </>
                                  )}
                                </Button>
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
                              </>
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
                                    setSaveError(getErrorMessage(error));
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
                  <h3 className="text-sm font-semibold text-foreground">执行历史明细</h3>
                  <span className="text-xs text-muted-foreground">
                    共 {filteredHistoryResults.length} 条
                  </span>
                  {retryMessage ? <Badge variant="secondary">{retryMessage}</Badge> : null}
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => void handleBatchRetry()}
                    disabled={
                      loading ||
                      isRunning ||
                      retryingResultKey !== null ||
                      retryableFilteredResults.length === 0
                    }
                  >
                    {batchRetryProgress ? (
                      <Spinner data-icon="inline-start" />
                    ) : (
                      <RotateCcwIcon data-icon="inline-start" />
                    )}
                    {batchRetryProgress
                      ? `批量重做 ${batchRetryProgress.completed}/${batchRetryProgress.total}`
                      : `批量重做 (${retryableFilteredResults.length})`}
                  </Button>
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
                        <TableHead className="w-24 text-center text-[11px] font-bold uppercase tracking-wider">
                          操作
                        </TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {loading ? (
                        <TableRow>
                          <TableCell colSpan={7} className="h-48 text-center">
                            <div className="flex flex-col items-center gap-2 text-muted-foreground">
                              <Spinner className="size-5" />
                              <span className="text-xs">正在调取数据库记录...</span>
                            </div>
                          </TableCell>
                        </TableRow>
                      ) : mergedHistoryResults.length === 0 ? (
                        <TableRow>
                          <TableCell
                            colSpan={7}
                            className="h-48 text-center text-xs italic text-muted-foreground"
                          >
                            暂无任何执行历史记录
                          </TableCell>
                        </TableRow>
                      ) : filteredHistoryResults.length === 0 ? (
                        <TableRow>
                          <TableCell
                            colSpan={7}
                            className="h-48 text-center text-xs italic text-muted-foreground"
                          >
                            当前日期筛选下暂无执行记录
                          </TableCell>
                        </TableRow>
                      ) : (
                        paginatedResults.map((item, idx) => {
                          const isSuccess = getResultSucceeded(item);
                          const resultKey = getResultKey(item);
                          const isRetrying = retryingResultKey === resultKey;
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
                                {isSuccess || item.result_id == null ? (
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
                              <TableCell className="py-3 text-center">
                                {isSuccess ? (
                                  <span className="text-xs text-muted-foreground">—</span>
                                ) : (
                                  <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={() => void handleRetryResult(item)}
                                    disabled={
                                      retryingResultKey !== null || batchRetryProgress !== null
                                    }
                                    aria-label={`重做门店 ${item.shop_code} 的失败请求`}
                                  >
                                    {isRetrying ? (
                                      <Spinner data-icon="inline-start" />
                                    ) : (
                                      <RotateCcwIcon data-icon="inline-start" />
                                    )}
                                    {isRetrying ? "重做中" : "重做"}
                                  </Button>
                                )}
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
                      onClick={() => setCurrentPage((page) => Math.max(1, page - 1))}
                      disabled={currentPage <= 1}
                    >
                      上一页
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setCurrentPage((page) => Math.min(totalPages, page + 1))}
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
