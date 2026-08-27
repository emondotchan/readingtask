import { useCallback, useEffect, useState } from "react";

import {
  getDailyTaskRunStatuses,
  getRuntimeStatus,
  pauseDailyTask,
  startDailyTaskBatch,
} from "@/api/commands";
import type {
  CommandError,
  DailyTaskRunSnapshot,
  RuntimeStatus,
  TaskItemResult,
  TaskRunSummary,
} from "@/types";

export type RunState =
  | "idle"
  | "not_started"
  | "running"
  | "paused"
  | "completed"
  | "monthly-completed"
  | "error";

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

function getCompletedRunState(error: CommandError): RunState | null {
  if (error.category !== "completed") {
    return null;
  }

  if (error.message.includes("该月度任务已全部完成")) {
    return "monthly-completed";
  }

  return "completed";
}

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

  const runtimeReady = Boolean(
    runtimeStatus?.openIdsReady &&
    runtimeStatus?.shopReady &&
    runtimeStatus?.provinceReady &&
    runtimeStatus?.fcReady,
  );

  const canSubmit = runtimeError === null && runtimeStatus !== null && runtimeReady;

  const applyRunSnapshots = useCallback((snapshots: DailyTaskRunSnapshot[]) => {
    setTaskRuns((previous) => {
      let next = previous;

      for (const snapshot of snapshots) {
        next = {
          ...next,
          [snapshot.task_id]: createTaskRunState(snapshot.task_id, {
            date: snapshot.date,
            runState: snapshot.run_state,
            processedCount: snapshot.processed_count,
            requestedCount: snapshot.requested_count,
            items: snapshot.items,
            summary: snapshot.summary,
            error: snapshot.error,
          }),
        };
      }

      return next;
    });
  }, []);

  const refreshRunSnapshots = useCallback(async () => {
    const snapshots = await getDailyTaskRunStatuses();
    applyRunSnapshots(snapshots);
  }, [applyRunSnapshots]);

  useEffect(() => {
    if (!canSubmit) {
      return;
    }

    void refreshRunSnapshots().catch(console.error);
    const intervalId = window.setInterval(() => {
      void refreshRunSnapshots().catch(console.error);
    }, 3000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [canSubmit, refreshRunSnapshots]);

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
        await startDailyTaskBatch([taskId], date);
        await refreshRunSnapshots();
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

        const completedRunState = getCompletedRunState(error);
        if (completedRunState) {
          setTaskRuns((previous) => ({
            ...previous,
            [taskId]: createTaskRunState(taskId, {
              date,
              runState: completedRunState,
              error: null,
            }),
          }));
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
    [canSubmit, refreshRunSnapshots],
  );

  const executeDailyBatch = useCallback(
    async (taskIds: string[], date: string) => {
      if (!canSubmit || taskIds.length === 0) {
        return;
      }

      setTaskRuns((previous) => {
        let next = previous;
        for (const taskId of taskIds) {
          const current = next[taskId] ?? createTaskRunState(taskId);
          if (current.runState === "running") {
            continue;
          }

          next = {
            ...next,
            [taskId]: createTaskRunState(taskId, {
              date,
              runState: "running",
            }),
          };
        }
        return next;
      });

      await startDailyTaskBatch(taskIds, date);
      await refreshRunSnapshots();
    },
    [canSubmit, refreshRunSnapshots],
  );

  const pauseDaily = useCallback(
    async (taskId: string) => {
      await pauseDailyTask(taskId).catch(console.error);
      setTaskRuns((previous) => {
        const current = previous[taskId] ?? createTaskRunState(taskId);
        return {
          ...previous,
          [taskId]: {
            ...current,
            runState: "paused",
            error: null,
          },
        };
      });
      await refreshRunSnapshots();
    },
    [refreshRunSnapshots],
  );

  const getTaskRun = useCallback((taskId: string) => taskRuns[taskId] ?? null, [taskRuns]);

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
    executeDailyBatch,
    pauseDaily,
    resetTaskRun,
  };
}

export type ReturnTypeUseMonthlyRunner = ReturnType<typeof useMonthlyRunner>;
