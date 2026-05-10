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
  getTaskDailyTasks,
  previewMonthlyTaskPlan,
  type FcRecord,
} from "@/api/commands";
import type {
  CourseRecord,
  MonthlyTask,
  DailyTask,
  MonthlyTaskPlanPreview,
} from "@/types";
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
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { ReturnTypeUseMonthlyRunner } from "@/features/useMonthlyRunner";
import { Badge } from "@/components/ui/badge";
import { cn, getErrorMessage } from "@/lib/utils";
import { TaskStatusDialog } from "./TaskStatusDialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type TaskType = MonthlyTask["task_type"];
type ParsedReadingLink = ReturnType<typeof parseReadingLink>;
type MonthlyTaskDraft = {
  taskType: TaskType;
  parsedReadingLink: NonNullable<ParsedReadingLink>;
  readingUrl: string;
  preview: MonthlyTaskPlanPreview;
};

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

function buildMonthlyTaskId(
  courseId: string,
  managerId: string,
  taskType: TaskType,
) {
  return `${getTaskMonthPrefix()}:${courseId.trim()}:${managerId.trim()}:${taskType}`;
}

function parseMultilineValues(raw: string) {
  const values: string[] = [];
  const seen = new Set<string>();

  for (const [index, line] of raw.split(/\r?\n/).entries()) {
    const value = (
      index === 0 ? line.trimStart().replace(/^\uFEFF/, "") : line
    ).trim();
    if (!value || seen.has(value)) {
      continue;
    }
    seen.add(value);
    values.push(value);
  }

  return values;
}

function getDailyTaskProgressTotal(task: DailyTask) {
  return task.shopcodes.length;
}

function isCompletedDay(task: DailyTask) {
  const progressTotal = getDailyTaskProgressTotal(task);
  return (
    task.run_status === "completed" ||
    task.is_locked ||
    (progressTotal > 0 && task.completed_count >= progressTotal)
  );
}

function getDailyTaskRunStatus(task: DailyTask | null | undefined) {
  return task?.run_status ?? "not_started";
}

function parseReadingLink(raw: string) {
  const readingUrl = raw.trim();
  if (!readingUrl) return null;

  try {
    const url = new URL(readingUrl);
    const redirectUri = url.searchParams.get("redirect_uri");
    const targetUrl = redirectUri ? new URL(redirectUri) : url;
    const courseId = targetUrl.searchParams.get("CourseID")?.trim();
    const managerId = targetUrl.searchParams.get("UID")?.trim();
    if (!courseId || !managerId) return null;

    return {
      courseId,
      managerId,
    };
  } catch {
    return null;
  }
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
  const [completedCountMap, setCompletedCountMap] = useState<
    Record<string, number>
  >({});
  const [completedDaysMap, setCompletedDaysMap] = useState<
    Record<string, number>
  >({});
  const [createError, setCreateError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [fcName, setFcName] = useState("");
  const [aveneReadingUrl, setAveneReadingUrl] = useState("");
  const [kloraneReadingUrl, setKloraneReadingUrl] = useState("");
  const [customShopcodesText, setCustomShopcodesText] = useState("");
  const [excludedOpenIdsText, setExcludedOpenIdsText] = useState("");
  const [taskPreviews, setTaskPreviews] = useState<
    Partial<Record<TaskType, MonthlyTaskPlanPreview>>
  >({});
  const [previewLoading, setPreviewLoading] = useState(false);

  const [selectedTask, setSelectedTask] = useState<MonthlyTask | null>(null);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [filterMonth, setFilterMonth] = useState(getCurrentMonth());
  const [fcFilter, setFcFilter] = useState("all");
  const [taskTypeFilter, setTaskTypeFilter] = useState<
    "all" | "Avene" | "Klorane"
  >("all");

  const [isStartingAll, setIsStartingAll] = useState(false);
  const startAllAbortController = useRef<AbortController | null>(null);

  const {
    runtimeStatus,
    runtimeReady,
    runtimeError,
    getTaskRun,
    executeDaily,
    executeDailyBatch,
    pauseDaily,
  } = currentRun;
  const runtimeConfigured = Boolean(runtimeStatus?.sqliteConfigured);

  const resetCreateForm = useCallback(() => {
    setFcName("");
    setAveneReadingUrl("");
    setKloraneReadingUrl("");
    setCustomShopcodesText("");
    setExcludedOpenIdsText("");
    setTaskPreviews({});
    setPreviewLoading(false);
    setCreateError(null);
  }, []);

  const loadData = useCallback(async () => {
    if (!runtimeConfigured) {
      setTasks([]);
      setFcs([]);
      setCourses([]);
      setProgressMap({});
      setCompletedCountMap({});
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
      setLoadError("FC 列表加载失败，请检查配置或稍后重试。");
      console.error("Failed to load FC list", fcsResult.reason);
    }

    if (coursesResult.status === "fulfilled") {
      setCourses(coursesResult.value);
    } else {
      setCourses([]);
      setLoadError(
        (previous) => previous ?? "课程列表加载失败，请检查配置或稍后重试。",
      );
      console.error("Failed to load course list", coursesResult.reason);
    }

    if (tasksResult.status !== "fulfilled") {
      setTasks([]);
      setProgressMap({});
      setCompletedCountMap({});
      setCompletedDaysMap({});
      setLoadError(
        (previous) =>
          previous ?? "月度计划列表加载失败，但仍可继续选择 FC 新建计划。",
      );
      console.error("Failed to load tasks", tasksResult.reason);
      return;
    }

    const ts = tasksResult.value;
    setTasks(ts);

    if (ts.length === 0) {
      setProgressMap({});
      setCompletedCountMap({});
      setCompletedDaysMap({});
      return;
    }

    const today = getTodayDate();
    const progressEntries: Array<readonly [string, DailyTask | null]> = [];
    const completedCountEntries: Array<readonly [string, number]> = [];
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
        const dailyTasks = await getTaskDailyTasks(task.id);
        completedCountEntries.push([
          task.id,
          dailyTasks.reduce((sum, item) => sum + item.completed_count, 0),
        ] as const);
        completedDaysEntries.push([
          task.id,
          dailyTasks.filter((item) => isCompletedDay(item)).length,
        ] as const);
      } catch (e) {
        console.error(`Failed to load daily tasks for task ${task.id}`, e);
      }
    }

    setProgressMap(Object.fromEntries(progressEntries));
    setCompletedCountMap(Object.fromEntries(completedCountEntries));
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
      new Set([
        currentMonth,
        ...tasks.map((task) => {
          const taskDate = new Date(task.created_at);
          return `${taskDate.getFullYear()}-${String(taskDate.getMonth() + 1).padStart(2, "0")}`;
        }),
      ]),
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

  const parsedReadingLinks = useMemo(
    () => ({
      Avene: parseReadingLink(aveneReadingUrl),
      Klorane: parseReadingLink(kloraneReadingUrl),
    }),
    [aveneReadingUrl, kloraneReadingUrl],
  );
  const selectedCourses = useMemo(
    () => ({
      Avene: parsedReadingLinks.Avene
        ? (availableCourses.find(
            (course) =>
              course.course_id === parsedReadingLinks.Avene?.courseId &&
              course.task_type === "Avene",
          ) ?? null)
        : null,
      Klorane: parsedReadingLinks.Klorane
        ? (availableCourses.find(
            (course) =>
              course.course_id === parsedReadingLinks.Klorane?.courseId &&
              course.task_type === "Klorane",
          ) ?? null)
        : null,
    }),
    [availableCourses, parsedReadingLinks],
  );
  const customShopcodes = useMemo(
    () => parseMultilineValues(customShopcodesText),
    [customShopcodesText],
  );
  const excludedOpenIds = useMemo(
    () => parseMultilineValues(excludedOpenIdsText),
    [excludedOpenIdsText],
  );

  useEffect(() => {
    if (!runtimeConfigured) {
      setTaskPreviews({});
      setPreviewLoading(false);
      return;
    }

    const fc = fcs.find((item) => item.name === fcName);
    if (!fc) {
      setTaskPreviews({});
      setPreviewLoading(false);
      return;
    }

    const previewInputs = (["Avene", "Klorane"] as const)
      .map((taskType) => {
        const parsedReadingLink = parsedReadingLinks[taskType];
        const selectedCourse = selectedCourses[taskType];
        const readingUrl =
          taskType === "Avene" ? aveneReadingUrl : kloraneReadingUrl;

        if (!parsedReadingLink || !selectedCourse) {
          return null;
        }

        const task: MonthlyTask = {
          id: buildMonthlyTaskId(
            parsedReadingLink.courseId,
            parsedReadingLink.managerId,
            taskType,
          ),
          fc_name: fc.name,
          s_manager_id: parsedReadingLink.managerId,
          s_course_id: parsedReadingLink.courseId,
          reading_url: readingUrl.trim(),
          task_type: taskType,
          total_target: 0,
          target_days: 0,
          created_at: new Date().toISOString(),
          shopcodes: customShopcodes,
          excluded_open_ids: excludedOpenIds,
        };

        return [taskType, task] as const;
      })
      .filter(
        (item): item is readonly [TaskType, MonthlyTask] => item !== null,
      );

    if (previewInputs.length === 0) {
      setTaskPreviews({});
      setPreviewLoading(false);
      return;
    }

    let cancelled = false;
    setPreviewLoading(true);
    Promise.all(
      previewInputs.map(async ([taskType, task]) => {
        const preview = await previewMonthlyTaskPlan(task);
        return [taskType, preview] as const;
      }),
    )
      .then((previews) => {
        if (!cancelled) {
          setTaskPreviews(Object.fromEntries(previews));
          setCreateError(null);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setTaskPreviews({});
          setCreateError(getErrorMessage(error));
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
  }, [
    aveneReadingUrl,
    customShopcodes,
    excludedOpenIds,
    fcName,
    fcs,
    kloraneReadingUrl,
    parsedReadingLinks,
    runtimeConfigured,
    selectedCourses,
  ]);

  const monthlyTaskDrafts = useMemo(() => {
    const fc = fcs.find((item) => item.name === fcName);
    if (!fc) {
      return [];
    }

    return (["Avene", "Klorane"] as const)
      .map((taskType): MonthlyTaskDraft | null => {
        const parsedReadingLink = parsedReadingLinks[taskType];
        const selectedCourse = selectedCourses[taskType];
        const preview = taskPreviews[taskType];
        const readingUrl =
          taskType === "Avene" ? aveneReadingUrl : kloraneReadingUrl;

        if (!parsedReadingLink || !selectedCourse || !preview) {
          return null;
        }

        return {
          taskType,
          parsedReadingLink,
          readingUrl,
          preview,
        };
      })
      .filter((draft): draft is MonthlyTaskDraft => draft !== null);
  }, [
    aveneReadingUrl,
    fcName,
    fcs,
    kloraneReadingUrl,
    parsedReadingLinks,
    selectedCourses,
    taskPreviews,
  ]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const fc = fcs.find((f) => f.name === fcName);
    if (!fc || monthlyTaskDrafts.length !== 2) return;

    try {
      const createdAt = new Date().toISOString();
      for (const draft of monthlyTaskDrafts) {
        const newTask: MonthlyTask = {
          id: buildMonthlyTaskId(
            draft.parsedReadingLink.courseId,
            draft.parsedReadingLink.managerId,
            draft.taskType,
          ),
          fc_name: fc.name,
          s_manager_id: draft.parsedReadingLink.managerId,
          s_course_id: draft.parsedReadingLink.courseId,
          reading_url: draft.readingUrl.trim(),
          task_type: draft.taskType,
          total_target: draft.preview.total_target,
          target_days: draft.preview.target_days,
          created_at: createdAt,
          shopcodes: customShopcodes,
          excluded_open_ids: excludedOpenIds,
        };
        await createMonthlyTask(newTask);
      }
      resetCreateForm();
      setCreateDialogOpen(false);
      await loadData();
    } catch (e) {
      setCreateError(getErrorMessage(e));
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
      const tasksToRun = tasks.filter((task) => {
        if ((completedDaysMap[task.id] ?? 0) >= task.target_days) return false;
        const progress = progressMap[task.id];
        if (progress && isCompletedDay(progress)) return false;
        const taskRun = getTaskRun(task.id);
        if (taskRun?.runState === "monthly-completed") return false;
        if (taskRun?.runState === "completed" && taskRun.date === todayDate)
          return false;
        if (taskRun?.runState === "running") return false;
        return true;
      });

      if (!signal.aborted) {
        await executeDailyBatch(
          tasksToRun.map((task) => task.id),
          todayDate,
        );
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
    <div className="flex w-full flex-col gap-6">
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
                    {Array.from(new Set(tasks.map((task) => task.fc_name)))
                      .sort()
                      .map((fcName) => (
                        <SelectItem
                          key={fcName}
                          value={fcName}
                          className="text-xs"
                        >
                          {fcName}
                        </SelectItem>
                      ))}
                  </SelectContent>
                </Select>
                <Select
                  value={taskTypeFilter}
                  onValueChange={(value) =>
                    setTaskTypeFilter(value as "all" | "Avene" | "Klorane")
                  }
                >
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
            <div className="max-h-[680px] overflow-auto">
              <Table className="min-w-[980px] table-fixed">
                <TableHeader>
                  <TableRow className="text-muted-foreground hover:bg-transparent">
                    <TableHead className="sticky top-0 z-10 w-[230px] bg-card px-5 text-left text-xs font-semibold uppercase shadow-[0_1px_0_0_var(--color-border)]">
                      任务
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 w-[180px] bg-card text-center text-xs font-semibold uppercase shadow-[0_1px_0_0_var(--color-border)]">
                      今日进度
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 w-[260px] bg-card text-center text-xs font-semibold uppercase shadow-[0_1px_0_0_var(--color-border)]">
                      月度进度
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 w-[150px] bg-card text-center text-xs font-semibold uppercase shadow-[0_1px_0_0_var(--color-border)]">
                      状态
                    </TableHead>
                    <TableHead className="sticky top-0 z-10 w-[220px] bg-card text-center text-xs font-semibold uppercase shadow-[0_1px_0_0_var(--color-border)]">
                      操作
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredTasks.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={5}
                        className="h-32 text-center text-muted-foreground"
                      >
                        {filterMonth} 暂无计划任务
                      </TableCell>
                    </TableRow>
                  ) : (
                    filteredTasks.map((task: MonthlyTask) => {
                      const prog = progressMap[task.id];
                      const taskRun = getTaskRun(task.id);
                      const dailyRunStatus = getDailyTaskRunStatus(prog);
                      const displayRunState =
                        taskRun?.runState ?? dailyRunStatus;
                      const isCompletedToday = Boolean(
                        (prog && isCompletedDay(prog)) ||
                        (displayRunState === "completed" &&
                          (taskRun?.date ?? today) === today),
                      );
                      const todayTarget = prog
                        ? getDailyTaskProgressTotal(prog)
                        : "-";
                      const todayCompleted = prog ? prog.completed_count : 0;
                      const completedCount = completedCountMap[task.id] ?? 0;
                      const completedDays = completedDaysMap[task.id] ?? 0;
                      const todayProgress =
                        typeof todayTarget === "number" && todayTarget > 0
                          ? Math.min(100, (todayCompleted / todayTarget) * 100)
                          : 0;
                      const monthlyProgress =
                        task.total_target > 0
                          ? Math.min(
                              100,
                              (completedCount / task.total_target) * 100,
                            )
                          : 0;
                      const dayProgress =
                        task.target_days > 0
                          ? Math.min(
                              100,
                              (completedDays / task.target_days) * 100,
                            )
                          : 0;
                      const isRunning = displayRunState === "running";
                      const isPaused = displayRunState === "paused";
                      const isCompletedMonth =
                        completedDays >= task.target_days ||
                        displayRunState === "monthly-completed";
                      const isError = displayRunState === "error";

                      return (
                        <TableRow
                          key={task.id}
                          className="group border-border/70 transition-colors hover:bg-muted/35"
                        >
                          <TableCell className="px-5 py-4 align-middle">
                            <div className="flex min-w-0 flex-col gap-2">
                              <div className="flex items-center gap-2">
                                <span className="truncate text-sm font-semibold text-foreground">
                                  {task.fc_name}
                                </span>
                                <Badge
                                  variant="outline"
                                  className={cn(
                                    "shrink-0",
                                    task.task_type === "Avene"
                                      ? "border-primary/20 bg-primary/10 text-primary dark:border-primary/30 dark:bg-primary/15 dark:text-primary"
                                      : "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300",
                                  )}
                                >
                                  {task.task_type}
                                </Badge>
                              </div>
                            </div>
                          </TableCell>
                          <TableCell className="px-4 text-center align-middle">
                            <div className="flex items-center justify-center gap-3">
                              <div className="w-16 shrink-0 h-1.5 overflow-hidden rounded-full bg-muted">
                                <div
                                  className={cn(
                                    "h-full transition-all duration-500",
                                    isCompletedToday
                                      ? "bg-emerald-500"
                                      : "bg-sky-500",
                                  )}
                                  style={{ width: `${todayProgress}%` }}
                                />
                              </div>
                              <div className="flex items-baseline justify-center gap-1 shrink-0">
                                <span
                                  className={cn(
                                    "text-sm font-bold tabular-nums",
                                    isCompletedToday
                                      ? "text-emerald-600 dark:text-emerald-400"
                                      : "text-foreground",
                                  )}
                                >
                                  {todayCompleted}
                                </span>
                                <span className="text-xs text-muted-foreground">
                                  / {todayTarget}
                                </span>
                              </div>
                            </div>
                          </TableCell>
                          <TableCell className="px-4 align-middle">
                            <div className="flex items-center justify-center gap-2">
                              <div className="flex items-center gap-1.5 rounded-lg bg-muted/50 px-2.5 py-1.5">
                                <span className="text-xs text-muted-foreground whitespace-nowrap">
                                  完成数量
                                </span>
                                <span className="text-sm font-semibold tabular-nums text-foreground whitespace-nowrap">
                                  {completedCount} / {task.total_target}
                                </span>
                              </div>
                              <div className="flex items-center gap-1.5 rounded-lg bg-muted/50 px-2.5 py-1.5">
                                <span className="text-xs text-muted-foreground whitespace-nowrap">
                                  完成天数
                                </span>
                                <span className="text-sm font-semibold tabular-nums text-foreground whitespace-nowrap">
                                  {completedDays} / {task.target_days}
                                </span>
                              </div>
                            </div>
                          </TableCell>
                          <TableCell className="text-center align-middle">
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
                                暂停
                              </Badge>
                            ) : isCompletedMonth ? (
                              <Badge
                                variant="outline"
                                className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300"
                              >
                                <CheckCircle2Icon className="mr-1 size-3" />
                                本月完成
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
                                已完成
                              </Badge>
                            ) : (
                              <Badge variant="secondary">未开始</Badge>
                            )}
                          </TableCell>
                          <TableCell className="px-4 text-center align-middle">
                            <div className="flex items-center justify-center gap-1.5">
                              {!isCompletedToday && !isCompletedMonth && (
                                <Button
                                  size="icon-sm"
                                  variant="default"
                                  className="shadow-sm"
                                  aria-label={
                                    isRunning
                                      ? "暂停执行"
                                      : isPaused
                                        ? "继续执行"
                                        : isError
                                          ? "重新执行"
                                          : "执行"
                                  }
                                  title={
                                    isRunning
                                      ? "暂停执行"
                                      : isPaused
                                        ? "继续执行"
                                        : isError
                                          ? "重新执行"
                                          : "执行"
                                  }
                                  onClick={() => {
                                    if (isRunning) {
                                      void pauseDaily(task.id);
                                      return;
                                    }
                                    void executeDaily(task.id, today);
                                  }}
                                >
                                  {isRunning ? (
                                    <SquareIcon />
                                  ) : isPaused ? (
                                    <PlayIcon />
                                  ) : isError ? (
                                    <PlayIcon />
                                  ) : (
                                    <PlayIcon />
                                  )}
                                </Button>
                              )}
                              <Button
                                variant="secondary"
                                size="icon-sm"
                                aria-label="查看详情"
                                title="查看详情"
                                onClick={() => handleShowStatus(task)}
                              >
                                <EyeIcon />
                              </Button>
                              <Button
                                variant="ghost"
                                size="icon-sm"
                                className="text-rose-500 hover:bg-rose-50 hover:text-rose-600 dark:text-rose-300 dark:hover:bg-rose-950/30 dark:hover:text-rose-200"
                                aria-label="删除计划"
                                title="删除计划"
                                onClick={() => handleDelete(task.id)}
                              >
                                <Trash2Icon />
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

      <Dialog
        open={createDialogOpen}
        onOpenChange={(nextOpen) => {
          setCreateDialogOpen(nextOpen);
          if (!nextOpen) {
            resetCreateForm();
          }
        }}
      >
        <DialogContent className="sm:max-w-3xl max-h-[88vh] overflow-hidden flex flex-col">
          <DialogHeader className="border-b pb-4">
            <DialogTitle className="flex items-center gap-2">
              <PlusIcon className="size-4 text-primary" />
              新建计划
            </DialogTitle>
            <DialogDescription>
              选择 FC 并分别输入 Avene 与 Klorane
              阅读链接，系统会一次创建两类月度任务。
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
                  <FieldLabel htmlFor="plan-fc">FC</FieldLabel>
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
                          fcs.length === 0 ? "暂无可选 FC" : "选择 FC"
                        }
                      />
                    </SelectTrigger>
                    <SelectContent>
                      {fcs.length === 0 ? (
                        <SelectItem value="__empty_fc__" disabled>
                          暂无 FC，请先在 FC 管理中维护数据
                        </SelectItem>
                      ) : (
                        fcs.map((fc) => (
                          <SelectItem key={fc.name} value={fc.name}>
                            {fc.name}
                          </SelectItem>
                        ))
                      )}
                    </SelectContent>
                  </Select>
                </Field>

                {(["Avene", "Klorane"] as const).map((taskType) => {
                  const readingUrl =
                    taskType === "Avene" ? aveneReadingUrl : kloraneReadingUrl;
                  const parsedReadingLink = parsedReadingLinks[taskType];
                  const selectedCourse = selectedCourses[taskType];
                  const fieldId = `plan-${taskType.toLowerCase()}-reading-url`;

                  return (
                    <Field key={taskType}>
                      <FieldLabel htmlFor={fieldId}>
                        {taskType} 阅读链接
                      </FieldLabel>
                      <Input
                        id={fieldId}
                        value={readingUrl}
                        placeholder="https://e-learning.eau-thermale-avene.cn/Common/QCSCoursePage.aspx?CourseID=..."
                        onChange={(event) =>
                          taskType === "Avene"
                            ? setAveneReadingUrl(event.target.value)
                            : setKloraneReadingUrl(event.target.value)
                        }
                      />
                      {readingUrl.trim() && !parsedReadingLink && (
                        <p className="text-xs text-destructive">
                          链接需包含 CourseID 与 UID，或在 redirect_uri 中包含
                          CourseID 与 UID。
                        </p>
                      )}
                      {parsedReadingLink && !selectedCourse && (
                        <p className="text-xs text-destructive">
                          当前月份课程配置中未找到 CourseID=
                          {parsedReadingLink.courseId} 且类型为 {taskType}{" "}
                          的课程。
                        </p>
                      )}
                      {parsedReadingLink && selectedCourse && (
                        <p className="text-xs text-muted-foreground">
                          已解析 CourseID={parsedReadingLink.courseId}，UID=
                          {parsedReadingLink.managerId}，任务类型=
                          {selectedCourse.task_type}
                        </p>
                      )}
                    </Field>
                  );
                })}

                <Field>
                  <FieldLabel htmlFor="plan-shopcodes">
                    自定义 Shopcodes
                  </FieldLabel>
                  <Textarea
                    id="plan-shopcodes"
                    rows={6}
                    className="max-h-48 overflow-y-auto"
                    placeholder={
                      "留空则按系统规则随机生成\n如需指定，请一行一个 shopcode"
                    }
                    value={customShopcodesText}
                    onChange={(event) =>
                      setCustomShopcodesText(event.target.value)
                    }
                  />
                </Field>

                <Field>
                  <FieldLabel htmlFor="plan-excluded-openids">
                    排除 OpenIDs
                  </FieldLabel>
                  <Textarea
                    id="plan-excluded-openids"
                    rows={6}
                    className="max-h-48 overflow-y-auto"
                    placeholder={
                      "可选，一行一个 OpenID\n执行时不会使用这些 OpenID"
                    }
                    value={excludedOpenIdsText}
                    onChange={(event) =>
                      setExcludedOpenIdsText(event.target.value)
                    }
                  />
                </Field>

                {previewLoading && (
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Spinner className="size-4" />
                    正在计算月度计划...
                  </div>
                )}

                {Object.keys(taskPreviews).length > 0 && (
                  <div className="flex flex-col gap-3">
                    {(customShopcodes.length > 0 ||
                      excludedOpenIds.length > 0) && (
                      <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                        {customShopcodes.length > 0 && (
                          <Badge variant="outline">
                            自定义 shopcodes {customShopcodes.length} 条
                          </Badge>
                        )}
                        {excludedOpenIds.length > 0 && (
                          <Badge variant="outline">
                            排除 openids {excludedOpenIds.length} 条
                          </Badge>
                        )}
                      </div>
                    )}
                    {(["Avene", "Klorane"] as const).map((taskType) => {
                      const preview = taskPreviews[taskType];
                      if (!preview) {
                        return null;
                      }

                      return (
                        <div
                          key={taskType}
                          className="flex flex-col gap-3 rounded-xl border border-border bg-muted/30 p-4"
                        >
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <Badge
                              variant="outline"
                              className={cn(
                                taskType === "Avene"
                                  ? "border-primary/20 bg-primary/10 text-primary"
                                  : "border-emerald-200 bg-emerald-50 text-emerald-700",
                              )}
                            >
                              {taskType}
                            </Badge>
                            <span className="text-sm text-muted-foreground">
                              符合条件门店 {preview.eligible_shop_count} 家。
                              任务目标 {preview.total_target} 家。 预计{" "}
                              {preview.target_days} 天完成。
                            </span>
                          </div>
                          <div className="max-h-48 space-y-2 overflow-y-auto pr-1">
                            {preview.daily_plans.map((plan) => (
                              <div
                                key={`${taskType}:${plan.date}`}
                                className="rounded-lg border border-border/70 bg-background/80 p-3 text-xs"
                              >
                                <div className="flex items-center justify-between gap-2">
                                  <span className="font-medium text-foreground">
                                    {plan.date}
                                  </span>
                                  <Badge variant="secondary">
                                    {plan.target_count} 家
                                  </Badge>
                                </div>
                                <div className="mt-1 break-all text-muted-foreground">
                                  {plan.shopcodes.join(", ")}
                                </div>
                              </div>
                            ))}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}

                <div className="flex items-center justify-end gap-2 pt-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => {
                      setCreateDialogOpen(false);
                      resetCreateForm();
                    }}
                  >
                    取消
                  </Button>
                  <Button
                    type="submit"
                    size="default"
                    className="h-10 shadow-sm"
                    disabled={
                      !runtimeReady ||
                      !!runtimeError ||
                      !fcName ||
                      monthlyTaskDrafts.length !== 2 ||
                      previewLoading
                    }
                  >
                    <PlusIcon className="mr-2 size-4" />
                    保存两类计划
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
