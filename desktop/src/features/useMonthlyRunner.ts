import { useCallback, useEffect, useRef, useState } from "react";

import {
  getRuntimeStatus,
  onTaskProgress,
  pauseDailyTask,
  runDailyTask,
} from "@/api/commands";
import type {
  CommandError,
  RuntimeStatus,
  TaskItemResult,
  TaskRunSummary,
} from "@/types";

export type RunState = "idle" | "running" | "paused" | "completed" | "error";

export interface MonthlyTaskRunState {
  taskId: string;
  date: string | null;
  runState: RunState;
  processedCount: number;
  requestedCount: number;
  items: TaskItemResult[];
  summary: TaskRunSummary | null;
  error: CommandError | null;
}

export type MonthlyTaskRuns = Record<string, MonthlyTaskRunState>;

function createTaskRunState(
  taskId: string,
  overrides: Partial<MonthlyTaskRunState> = {},
): MonthlyTaskRunState {
  return {
    taskId,
    date: null,
    runState: "idle",
    processedCount: 0,
    requestedCount: 0,
    items: [],
    summary: null,
    error: null,
    ...overrides,
  };
}

export function useMonthlyRunner() {
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [taskRuns, setTaskRuns] = useState<MonthlyTaskRuns>({});

  const unlistenRef = useRef<(() => void) | null>(null);

  const refreshRuntimeStatus = useCallback(async () => {
    await getRuntimeStatus()
      .then((status) => {
        setRuntimeStatus(status);
        setRuntimeError(null);
      })
      .catch((reason) => {
        setRuntimeError(typeof reason === "string" ? reason : String(reason));
      });
  }, []);

  useEffect(() => {
    void refreshRuntimeStatus();
  }, [refreshRuntimeStatus]);

  useEffect(() => {
    let cancelled = false;

    onTaskProgress((progress) => {
      if (cancelled || !progress.task_id) {
        return;
      }

      setTaskRuns((previous) => {
        const current =
          previous[progress.task_id!] ?? createTaskRunState(progress.task_id!);

        return {
          ...previous,
          [progress.task_id!]: {
            ...current,
            runState: "running",
            processedCount: progress.processed_count,
            requestedCount: progress.requested_count,
            items: [...current.items, progress.latest_item],
            error: null,
          },
        };
      });
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
      } else {
        unlistenRef.current = unlisten;
      }
    });

    return () => {
      cancelled = true;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  const runtimeReady = Boolean(
    runtimeStatus?.openIdsReady &&
      runtimeStatus?.shopReady &&
      runtimeStatus?.provinceReady &&
      runtimeStatus?.fcReady,
  );

  const canSubmit =
    runtimeError === null && runtimeStatus !== null && runtimeReady;

  const executeDaily = useCallback(
    async (taskId: string, date: string) => {
      if (!canSubmit) {
        return;
      }

      let shouldStart = true;
      setTaskRuns((previous) => {
        const current = previous[taskId] ?? createTaskRunState(taskId);
        if (current.runState === "running") {
          shouldStart = false;
          return previous;
        }

        return {
          ...previous,
          [taskId]: createTaskRunState(taskId, {
            date,
            runState: "running",
          }),
        };
      });

      if (!shouldStart) {
        return;
      }

      try {
        const result = await runDailyTask(taskId, date);
        setTaskRuns((previous) => ({
          ...previous,
          [taskId]: createTaskRunState(taskId, {
            date,
            runState: "completed",
            processedCount: result.processed_count,
            requestedCount: result.requested_count,
            items: result.items,
            summary: result,
            error: null,
          }),
        }));
      } catch (reason: unknown) {
        const maybeCommandError = reason as CommandError;
        const error =
          maybeCommandError &&
          typeof maybeCommandError === "object" &&
          "category" in maybeCommandError
            ? maybeCommandError
            : { category: "execution", message: String(reason) };

        if (error.category === "paused") {
          setTaskRuns((previous) => {
            const current = previous[taskId] ?? createTaskRunState(taskId);
            return {
              ...previous,
              [taskId]: {
                ...current,
                date,
                runState: "paused",
                error: null,
              },
            };
          });
          return;
        }

        setTaskRuns((previous) => {
          const current = previous[taskId] ?? createTaskRunState(taskId);
          return {
            ...previous,
            [taskId]: {
              ...current,
              date,
              runState: "error",
              error,
            },
          };
        });
      }
    },
    [canSubmit],
  );

  const pauseDaily = useCallback(async (taskId: string) => {
    await pauseDailyTask(taskId);
  }, []);

  const getTaskRun = useCallback(
    (taskId: string) => taskRuns[taskId] ?? null,
    [taskRuns],
  );

  const resetTaskRun = useCallback((taskId: string) => {
    setTaskRuns((previous) => {
      if (!previous[taskId]) {
        return previous;
      }

      return {
        ...previous,
        [taskId]: createTaskRunState(taskId),
      };
    });
  }, []);

  return {
    runtimeStatus,
    runtimeError,
    runtimeReady,
    taskRuns,
    getTaskRun,
    refreshRuntimeStatus,
    canSubmit,
    executeDaily,
    pauseDaily,
    resetTaskRun,
  };
}

export type ReturnTypeUseMonthlyRunner = ReturnType<typeof useMonthlyRunner>;
