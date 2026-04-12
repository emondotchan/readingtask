import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  PlusIcon,
  Trash2Icon,
  PlayIcon,
  SquareIcon,
  ActivityIcon,
  EyeIcon,
  CheckCircle2Icon,
  AlertCircleIcon,
} from "lucide-react";
import {
  getMonthlyTasks,
  createMonthlyTask,
  deleteMonthlyTask,
  getCourses,
  getDailyTask,
  getFcs,
  getTaskResults,
  previewMonthlyTaskPlan,
  type FcRecord,
} from "@/api/commands";
import type { CourseRecord, MonthlyTask, DailyTask, MonthlyTaskPlanPreview } from "@/types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import type { ReturnTypeUseMonthlyRunner } from "@/features/useMonthlyRunner";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { TaskStatusDialog } from "./TaskStatusDialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

function getTodayDate() {
  const d = new Date();
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function getCurrentMonth() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function getTaskMonthPrefix(date = new Date()) {
  const year = String(date.getFullYear()).slice(-2);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  return `${year}${month}`;
}

function buildMonthlyTaskId(courseId: string, managerId: string) {
  return `${getTaskMonthPrefix()}:${courseId.trim()}:${managerId.trim()}`;
}

function buildCourseOptionValue(course: CourseRecord) {
  return `${course.month}:${course.task_type}:${course.course_id}`;
}

function getResultSucceeded(item: { submit_err?: number | null; outcome: string }) {
  return item.submit_err === 0
    ? true
    : typeof item.submit_err === "number"
      ? false
      : item.outcome === "Success";
}

export default function MonthlyPlanManager({
  currentRun,
}: {
  currentRun: ReturnTypeUseMonthlyRunner;
}) {
  const [tasks, setTasks] = useState<MonthlyTask[]>([]);
  const [fcs, setFcs] = useState<FcRecord[]>([]);
  const [courses, setCourses] = useState<CourseRecord[]>([]);
  const [progressMap, setProgressMap] = useState<
    Record<string, DailyTask | null>
  >({});
  const [successCountMap, setSuccessCountMap] = useState<
    Record<string, number>
  >({});
  const [completedDaysMap, setCompletedDaysMap] = useState<
    Record<string, number>
  >({});
  const [createError, setCreateError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [fcName, setFcName] = useState("");
  const [selectedCourseKey, setSelectedCourseKey] = useState("");
  const [taskPreview, setTaskPreview] = useState<MonthlyTaskPlanPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const [selectedTask, setSelectedTask] = useState<MonthlyTask | null>(null);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [filterMonth, setFilterMonth] = useState(getCurrentMonth());
  const [fcFilter, setFcFilter] = useState("all");
  const [taskTypeFilter, setTaskTypeFilter] = useState<"all" | "Avene" | "Klorane">("all");

  const [isStartingAll, setIsStartingAll] = useState(false);
  const startAllAbortController = useRef<AbortController | null>(null);

  const { runtimeStatus, runtimeReady, runtimeError, getTaskRun, executeDaily, pauseDaily } =
    currentRun;
  const runtimeConfigured = Boolean(runtimeStatus?.sqliteConfigured);

  const loadData = useCallback(async () => {
    if (!runtimeConfigured) {
      setTasks([]);
      setFcs([]);
      setCourses([]);
      setProgressMap({});
      setSuccessCountMap({});
      setCompletedDaysMap({});
      setLoadError(null);
      return;
    }

    setLoadError(null);

    const fcsResult = await getFcs()
      .then((value) => ({ status: "fulfilled", value }) as const)
      .catch((reason) => ({ status: "rejected", reason }) as const);

    const coursesResult = await getCourses()
      .then((value) => ({ status: "fulfilled", value }) as const)
      .catch((reason) => ({ status: "rejected", reason }) as const);

    const tasksResult = await getMonthlyTasks()
      .then((value) => ({ status: "fulfilled", value }) as const)
      .catch((reason) => ({ status: "rejected", reason }) as const);

    if (fcsResult.status === "fulfilled") {
      setFcs(fcsResult.value);
    } else {
      setFcs([]);
      setLoadError("FC 经理列表加载失败，请检查配置或稍后重试。");
      console.error("Failed to load FC list", fcsResult.reason);
    }

    if (coursesResult.status === "fulfilled") {
      setCourses(coursesResult.value);
    } else {
      setCourses([]);
      setLoadError((previous) => previous ?? "课程列表加载失败，请检查配置或稍后重试。");
      console.error("Failed to load course list", coursesResult.reason);
    }

    if (tasksResult.status !== "fulfilled") {
      setTasks([]);
      setProgressMap({});
      setSuccessCountMap({});
      setCompletedDaysMap({});
      setLoadError(
        (previous) =>
          previous ?? "月度计划列表加载失败，但仍可继续选择 FC 经理新建计划。",
      );
      console.error("Failed to load tasks", tasksResult.reason);
      return;
    }

    const ts = tasksResult.value;
    setTasks(ts);

    if (ts.length === 0) {
      setProgressMap({});
      setSuccessCountMap({});
      setCompletedDaysMap({});
      return;
    }

    const today = getTodayDate();
    const progressEntries: Array<readonly [string, DailyTask | null]> = [];
    const successCountEntries: Array<readonly [string, number]> = [];
    const completedDaysEntries: Array<readonly [string, number]> = [];

    for (const task of ts) {
      try {
        const progress = await getDailyTask(task.id, today);
        progressEntries.push([task.id, progress] as const);
      } catch (e) {
        console.error(`Failed to load progress for task ${task.id}`, e);
      }
    }

    for (const task of ts) {
      try {
        const results = await getTaskResults(task.id);
        const successResults = results.filter((item) => getResultSucceeded(item));
        successCountEntries.push([
          task.id,
          successResults.length,
        ] as const);
        completedDaysEntries.push([
          task.id,
          new Set(
            successResults
              .map((item) => {
                const dt = item.executed_date;
                if (!dt) return null;
                return dt.split(" ")[0]; // normalize to YYYY-MM-DD
              })
              .filter((value): value is string => Boolean(value)),
          ).size,
        ] as const);
      } catch (e) {
        console.error(`Failed to load results for task ${task.id}`, e);
      }
    }

    setProgressMap(Object.fromEntries(progressEntries));
    setSuccessCountMap(Object.fromEntries(successCountEntries));
    setCompletedDaysMap(Object.fromEntries(completedDaysEntries));
  }, [runtimeConfigured]);

  useEffect(() => {
    void loadData();
    if (!runtimeConfigured) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadData();
    }, 5000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [loadData, runtimeConfigured]);

  const monthOptions = useMemo(() => {
    const currentMonth = getCurrentMonth();
    return Array.from(
      new Set(
        [
          currentMonth,
          ...tasks.map((task) => {
            const taskDate = new Date(task.created_at);
            return `${taskDate.getFullYear()}-${String(taskDate.getMonth() + 1).padStart(2, "0")}`;
          }),
        ],
      ),
    )
      .sort((a, b) => b.localeCompare(a))
      .map((value) => ({
        value,
        label: `${value.slice(0, 4)}年${value.slice(5, 7)}月`,
      }));
  }, [tasks]);

  const filteredTasks = useMemo(() => {
    return tasks.filter((task) => {
      const taskDate = new Date(task.created_at);
      const taskMonth = `${taskDate.getFullYear()}-${String(taskDate.getMonth() + 1).padStart(2, "0")}`;
      const matchesMonth = taskMonth === filterMonth;
      const matchesFc = fcFilter === "all" || task.fc_name === fcFilter;
      const matchesTaskType =
        taskTypeFilter === "all" || task.task_type === taskTypeFilter;

      return matchesMonth && matchesFc && matchesTaskType;
    });
  }, [tasks, filterMonth, fcFilter, taskTypeFilter]);

  const availableCourses = useMemo(() => {
    const currentMonth = getCurrentMonth();
    return courses.filter((course) => course.month === currentMonth);
  }, [courses]);

  const selectedCourse =
    availableCourses.find((course) => buildCourseOptionValue(course) === selectedCourseKey) ?? null;

  useEffect(() => {
    if (!runtimeConfigured) {
      setTaskPreview(null);
      setPreviewLoading(false);
      return;
    }

    const fc = fcs.find((item) => item.name === fcName);
    if (!fc) {
      setTaskPreview(null);
      setPreviewLoading(false);
      return;
    }

    if (!selectedCourse) {
      setTaskPreview(null);
      setPreviewLoading(false);
      return;
    }

    const previewTask: MonthlyTask = {
      id: buildMonthlyTaskId(selectedCourse.course_id, fc.manager_id),
      fc_name: fc.name,
      s_manager_id: fc.manager_id,
      s_course_id: selectedCourse.course_id,
      task_type: selectedCourse.task_type,
      total_target: 0,
      target_days: 0,
      created_at: new Date().toISOString(),
      shopcodes: [],
    };

    let cancelled = false;
    setPreviewLoading(true);
    previewMonthlyTaskPlan(previewTask)
      .then((preview) => {
        if (!cancelled) {
          setTaskPreview(preview);
          setCreateError(null);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setTaskPreview(null);
          setCreateError(error instanceof Error ? error.message : String(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setPreviewLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [fcName, fcs, runtimeConfigured, selectedCourse]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const fc = fcs.find((f) => f.name === fcName);
    if (!fc || !selectedCourse || !taskPreview) return;

    const newTask: MonthlyTask = {
      id: buildMonthlyTaskId(selectedCourse.course_id, fc.manager_id),
      fc_name: fc.name,
      s_manager_id: fc.manager_id,
      s_course_id: selectedCourse.course_id,
      task_type: selectedCourse.task_type,
      total_target: taskPreview.total_target,
      target_days: taskPreview.target_days,
      created_at: new Date().toISOString(),
      shopcodes: [],
    };

    try {
      await createMonthlyTask(newTask);
      setCreateError(null);
      setFcName("");
      setSelectedCourseKey("");
      setTaskPreview(null);
      setCreateDialogOpen(false);
      await loadData();
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : String(e));
      console.error(e);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteMonthlyTask(id);
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleStartAll = async () => {
    if (startAllAbortController.current) return;
    const controller = new AbortController();
    startAllAbortController.current = controller;
    const signal = controller.signal;
    setIsStartingAll(true);

    try {
      const todayDate = getTodayDate();
      const tasksToRun = filteredTasks.filter((task) => {
        const progress = progressMap[task.id];
        if (progress?.is_locked) return false;
        if (progress && progress.completed_count >= progress.target_count) return false;
        const taskRun = getTaskRun(task.id);
        if (taskRun?.runState === "running") return false;
        return true;
      });

      for (let i = 0; i < tasksToRun.length; i++) {
        if (signal.aborted) break;

        const task = tasksToRun[i];
        void executeDaily(task.id, todayDate).catch(console.error);

        if (i < tasksToRun.length - 1) {
          // Use a short stagger so all runnable tasks are queued quickly
          // without forcing the user to wait minutes between starts.
          const waitMs = Math.floor(Math.random() * (2500 - 1200 + 1)) + 1200;

          await new Promise<void>((resolve) => {
            const onAbort = () => {
              window.clearTimeout(timeout);
              signal.removeEventListener("abort", onAbort);
              resolve();
            };
            const timeout = window.setTimeout(() => {
              signal.removeEventListener("abort", onAbort);
              resolve();
            }, waitMs);

            signal.addEventListener("abort", onAbort, { once: true });
          });
        }
      }
    } finally {
      setIsStartingAll(false);
      startAllAbortController.current = null;
    }
  };

  const handleCancelStartAll = () => {
    if (startAllAbortController.current) {
      startAllAbortController.current.abort();
      startAllAbortController.current = null;
      setIsStartingAll(false);
    }
  };

  const handleShowStatus = (task: MonthlyTask) => {
    setSelectedTask(task);
  };

  const today = getTodayDate();

  return (
    <div className="flex w-full max-w-6xl mx-auto flex-col gap-6">
      {selectedTask ? (
        <TaskStatusDialog
          task={selectedTask}
          onBack={() => setSelectedTask(null)}
          currentRun={getTaskRun(selectedTask.id) ?? undefined}
        />
      ) : (
        <Card className="min-h-[400px] overflow-hidden border-border shadow-sm transition-all hover:border-primary/30">
          <CardHeader className="border-b border-border pb-3">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex items-center gap-2">
                <ActivityIcon className="size-4 text-emerald-500" />
                <CardTitle className="text-lg font-semibold">
                  月度计划
                </CardTitle>
                <Badge variant="secondary" className="font-mono">
                  {filteredTasks.length}
                </Badge>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {isStartingAll ? (
                  <Button
                    size="sm"
                    variant="destructive"
                    className="h-8 shadow-sm"
                    onClick={handleCancelStartAll}
                  >
                    <SquareIcon className="mr-1 size-3.5" />
                    停止一键执行
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    className="h-8 shadow-sm"
                    onClick={handleStartAll}
                    disabled={!runtimeReady || !!runtimeError}
                  >
                    <PlayIcon className="mr-1 size-3.5" />
                    一键开始
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="outline"
                  className="h-8 shadow-sm"
                  onClick={() => setCreateDialogOpen(true)}
                >
                  <PlusIcon className="mr-1 size-3.5" />
                  新建计划
                </Button>
                <Select value={filterMonth} onValueChange={setFilterMonth}>
                  <SelectTrigger className="h-8 w-[140px] border-border bg-background/70 text-xs">
                    <SelectValue placeholder="筛选月份" />
                  </SelectTrigger>
                  <SelectContent>
                    {monthOptions.map(
                      (opt: { label: string; value: string }) => (
                        <SelectItem
                          key={opt.value}
                          value={opt.value}
                          className="text-xs"
                        >
                          {opt.label}
                        </SelectItem>
                      ),
                    )}
                  </SelectContent>
                </Select>
                <Select value={fcFilter} onValueChange={setFcFilter}>
                  <SelectTrigger className="h-8 w-[140px] border-border bg-background/70 text-xs">
                    <SelectValue placeholder="筛选 FC" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部 FC</SelectItem>
                    {Array.from(new Set(tasks.map((task) => task.fc_name))).sort().map((fcName) => (
                      <SelectItem key={fcName} value={fcName} className="text-xs">
                        {fcName}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Select value={taskTypeFilter} onValueChange={(value) => setTaskTypeFilter(value as "all" | "Avene" | "Klorane")}>
                  <SelectTrigger className="h-8 w-[140px] border-border bg-background/70 text-xs">
                    <SelectValue placeholder="筛选任务类型" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部类型</SelectItem>
                    <SelectItem value="Avene">Avene</SelectItem>
                    <SelectItem value="Klorane">Klorane</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            {loadError && (
              <Alert className="mt-3">
                <AlertTitle>数据加载异常</AlertTitle>
                <AlertDescription>{loadError}</AlertDescription>
              </Alert>
            )}
          </CardHeader>
          <CardContent className="p-0">
            <div className="max-h-[640px] overflow-auto">
              <Table>
                <TableHeader>
                  <TableRow className="text-muted-foreground hover:bg-transparent">
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      FC 经理
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      今日进度
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      任务类型
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      累计完成
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      月度总目标
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      已完成天数
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider shadow-[0_1px_0_0_var(--color-border)]">
                      状态
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 bg-card text-center font-semibold text-xs uppercase tracking-wider w-[220px] shadow-[0_1px_0_0_var(--color-border)]">
                      操作
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredTasks.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={8}
                        className="h-32 text-center text-muted-foreground"
                      >
                        {filterMonth} 暂无计划任务
                      </TableCell>
                    </TableRow>
                  ) : (
                    filteredTasks.map((task: MonthlyTask) => {
                      const prog = progressMap[task.id];
                      const taskRun = getTaskRun(task.id);
                      const isCompletedToday = Boolean(
                        prog && (prog.is_locked || prog.completed_count >= prog.target_count),
                      );
                      const todayTarget = prog ? prog.target_count : "-";
                      const todayCompleted = prog ? prog.completed_count : 0;
                      const successCount = successCountMap[task.id] ?? 0;
                      const completedDays = completedDaysMap[task.id] ?? 0;
                      const isRunning = taskRun?.runState === "running";
                      const isPaused = taskRun?.runState === "paused";
                      const isError = taskRun?.runState === "error";

                  return (
                    <TableRow
                      key={task.id}
                      className="group border-border/70 transition-colors hover:bg-muted/35"
                    >
                      <TableCell className="py-4 text-center">
                        <span className="font-semibold text-foreground">
                          {task.fc_name}
                        </span>
                      </TableCell>
                      <TableCell className="text-center">
                        <div className="flex flex-col items-center gap-1.5 min-w-24">
                          <div className="flex items-baseline gap-1">
                            <span
                              className={cn(
                                "text-sm font-bold",
                                isCompletedToday
                                  ? "text-emerald-600 dark:text-emerald-400"
                                  : "text-foreground",
                              )}
                            >
                              {todayCompleted}
                            </span>
                            <span className="text-[10px] text-muted-foreground">
                              / {todayTarget}
                            </span>
                          </div>
                          <div className="h-1.5 w-24 overflow-hidden rounded-full bg-muted">
                            <div
                              className={cn(
                                "h-full transition-all duration-500",
                                isCompletedToday
                                  ? "bg-emerald-500"
                                  : "bg-sky-500",
                              )}
                              style={{
                                width: `${Math.min(100, (todayCompleted / (typeof todayTarget === "number" ? todayTarget : 1)) * 100)}%`,
                              }}
                            />
                          </div>
                        </div>
                      </TableCell>
                      <TableCell className="text-center">
                        <Badge
                          variant="outline"
                          className={cn(
                            task.task_type === "Avene"
                              ? "border-primary/20 bg-primary/10 text-primary dark:border-primary/30 dark:bg-primary/15 dark:text-primary"
                              : "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300",
                          )}
                        >
                          {task.task_type}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-center">
                        <div className="flex flex-col items-center">
                          <span className="text-sm font-semibold text-foreground">
                            {successCount}
                          </span>
                          <span className="text-[10px] text-muted-foreground">
                            成功记录
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className="text-center">
                        <div className="flex flex-col items-center">
                          <span className="text-sm font-semibold text-foreground">
                            {task.total_target}
                          </span>
                          <span className="text-[10px] text-muted-foreground">
                            分 {task.target_days} 天完成
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className="text-center">
                        <div className="flex flex-col items-center">
                          <span className="text-sm font-semibold text-foreground">
                            {completedDays}
                          </span>
                          <span className="text-[10px] text-muted-foreground">
                            / {task.target_days} 天
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className="text-center">
                        {isRunning ? (
                          <Badge
                            variant="outline"
                            className="border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-900/60 dark:bg-sky-950/20 dark:text-sky-300"
                          >
                            <Spinner className="mr-1 size-3" />
                            执行中
                          </Badge>
                        ) : isPaused ? (
                          <Badge
                            variant="outline"
                            className="border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900/60 dark:bg-amber-950/20 dark:text-amber-300"
                          >
                            <AlertCircleIcon className="mr-1 size-3" />
                            已暂停
                          </Badge>
                        ) : isError ? (
                          <Badge
                            variant="outline"
                            className="border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/60 dark:bg-rose-950/20 dark:text-rose-300"
                          >
                            <AlertCircleIcon className="mr-1 size-3" />
                            执行失败
                          </Badge>
                        ) : isCompletedToday ? (
                          <Badge
                            variant="outline"
                            className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300"
                          >
                            <CheckCircle2Icon className="mr-1 size-3" />
                            今日完成
                          </Badge>
                        ) : (
                          <Badge variant="secondary">待执行</Badge>
                        )}
                      </TableCell>
                      <TableCell className="text-center">
                        <div className="flex items-center justify-center gap-1.5 px-2">
                          {!isCompletedToday && (
                            <Button
                              size="sm"
                              variant="default"
                              className="h-8 text-xs shadow-sm"
                              onClick={() => {
                                if (isRunning) {
                                  void pauseDaily(task.id);
                                  return;
                                }
                                void executeDaily(task.id, today);
                              }}
                            >
                              {isRunning ? (
                                "暂停执行"
                              ) : isPaused ? (
                                "继续执行"
                              ) : isError ? (
                                "重新执行"
                              ) : (
                                <>
                                  <PlayIcon className="size-3 mr-1" />
                                  执行
                                </>
                              )}
                            </Button>
                          )}
                          <Button
                            variant="secondary"
                            size="sm"
                            className="h-8 text-xs"
                            onClick={() => handleShowStatus(task)}
                          >
                            <EyeIcon className="size-3 mr-1" /> 详情
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-8 text-xs text-rose-500 hover:bg-rose-50 hover:text-rose-600 dark:text-rose-300 dark:hover:bg-rose-950/30 dark:hover:text-rose-200"
                            onClick={() => handleDelete(task.id)}
                          >
                            <Trash2Icon className="size-3 mr-1" /> 删除
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                    })
                  )}
                </TableBody>
              </Table>
            </div>
          </CardContent>
        </Card>
      )}

      <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
        <DialogContent className="sm:max-w-2xl max-h-[88vh] overflow-hidden flex flex-col">
          <DialogHeader className="border-b pb-4">
            <DialogTitle className="flex items-center gap-2">
              <PlusIcon className="size-4 text-primary" />
              新建计划
            </DialogTitle>
            <DialogDescription>
              选择 FC 与已配置课程，系统会预生成月度执行计划。
            </DialogDescription>
          </DialogHeader>
          <div className="flex-1 overflow-y-auto py-2 pr-1">
            <form className="flex flex-col gap-5" onSubmit={handleCreate}>
              {createError && (
                <Alert variant="destructive">
                  <AlertTitle>保存失败</AlertTitle>
                  <AlertDescription>{createError}</AlertDescription>
                </Alert>
              )}
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="plan-course">课程</FieldLabel>
                  <Select
                    value={selectedCourseKey}
                    onValueChange={setSelectedCourseKey}
                    disabled={availableCourses.length === 0}
                  >
                    <SelectTrigger
                      id="plan-course"
                      className="w-full data-[size=default]:h-9"
                    >
                      <SelectValue
                        placeholder={
                          availableCourses.length === 0
                            ? "当前月份暂无可选课程"
                            : "选择课程"
                        }
                      />
                    </SelectTrigger>
                    <SelectContent>
                      {availableCourses.length === 0 ? (
                        <SelectItem value="__empty_course__" disabled>
                          请先在课程管理中维护本月课程
                        </SelectItem>
                      ) : (
                        availableCourses.map((course) => (
                          <SelectItem
                            key={buildCourseOptionValue(course)}
                            value={buildCourseOptionValue(course)}
                          >
                            {course.month} · {course.task_type} · {course.course_id}
                          </SelectItem>
                        ))
                      )}
                    </SelectContent>
                  </Select>
                </Field>

                <Field>
                  <FieldLabel htmlFor="plan-fc">FC 经理</FieldLabel>
                  <Select
                    value={fcName}
                    onValueChange={setFcName}
                    disabled={fcs.length === 0}
                  >
                    <SelectTrigger
                      id="plan-fc"
                      className="w-full data-[size=default]:h-9"
                    >
                      <SelectValue
                        placeholder={
                          fcs.length === 0 ? "暂无可选经理" : "选择经理"
                        }
                      />
                    </SelectTrigger>
                    <SelectContent>
                      {fcs.length === 0 ? (
                        <SelectItem value="__empty_fc__" disabled>
                          暂无 FC 经理，请先在经理管理中维护数据
                        </SelectItem>
                      ) : (
                        fcs.map((fc) => (
                          <SelectItem key={fc.name} value={fc.name}>
                            {fc.name} ({fc.manager_id})
                          </SelectItem>
                        ))
                      )}
                    </SelectContent>
                  </Select>
                </Field>

                {previewLoading && (
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Spinner className="size-4" />
                    正在计算月度计划...
                  </div>
                )}

                {taskPreview && (
                  <div className="space-y-3 rounded-xl border border-border bg-muted/30 p-4">
                    <div className="text-sm text-muted-foreground">
                      符合条件门店 {taskPreview.eligible_shop_count} 家。
                      任务目标 {taskPreview.total_target} 家。
                      预计 {taskPreview.target_days} 天完成。
                    </div>
                    <div className="max-h-56 space-y-2 overflow-y-auto pr-1">
                      {taskPreview.daily_plans.map((plan) => (
                        <div
                          key={plan.date}
                          className="rounded-lg border border-border/70 bg-background/80 p-3 text-xs"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="font-medium text-foreground">{plan.date}</span>
                            <Badge variant="secondary">{plan.target_count} 家</Badge>
                          </div>
                          <div className="mt-1 break-all text-muted-foreground">
                            {plan.shopcodes.join(", ")}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                <div className="flex items-center justify-end gap-2 pt-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => setCreateDialogOpen(false)}
                  >
                    取消
                  </Button>
                  <Button
                    type="submit"
                    size="default"
                    className="h-10 shadow-sm"
                    disabled={
                      !runtimeReady || !!runtimeError || !selectedCourse || !fcName || !taskPreview || previewLoading
                    }
                  >
                    <PlusIcon className="mr-2 size-4" />
                    保存计划任务
                  </Button>
                </div>
              </FieldGroup>
            </form>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
