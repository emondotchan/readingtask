import { useCallback, useEffect, useRef, useState } from "react";

import {
  getDailyProgress,
  getTaskResults,
  getRuntimeStatus,
  onTaskProgress,
  runReadingTask,
} from "@/api/commands";
import { parseShopcodesInput } from "@/lib/utils";
import type {
  CommandError,
  DailyProgress,
  RuntimeStatus,
  TaskItemResult,
  TaskRunSummary,
} from "@/types";

export type RunState = "idle" | "running" | "completed" | "error";

export interface FormState {
  sCourseId: string;
  sManagerId: string;
  fc: string;
  shopcodesInput: string;
}

const defaultForm: FormState = {
  sCourseId: "",
  sManagerId: "",
  fc: "",
  shopcodesInput: "",
};

function getTodayDate() {
  const d = new Date();
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function useTaskRunner() {
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(defaultForm);
  const [runState, setRunState] = useState<RunState>("idle");
  const [processedCount, setProcessedCount] = useState(0);
  const [requestedCount, setRequestedCount] = useState(0);
  const [items, setItems] = useState<TaskItemResult[]>([]);
  const [summary, setSummary] = useState<TaskRunSummary | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [linkedTaskId, setLinkedTaskId] = useState<string | null>(null);
  const [linkedTaskProgress, setLinkedTaskProgress] = useState<DailyProgress | null>(null);
  const [linkedTaskResults, setLinkedTaskResults] = useState<TaskItemResult[] | null>(null);
  const [linkedTaskSyncError, setLinkedTaskSyncError] = useState<string | null>(null);

  const unlistenRef = useRef<(() => void) | null>(null);

  const refreshRuntimeStatus = useCallback(async () => {
    await getRuntimeStatus()
      .then((status) => {
        setRuntimeStatus(status);
        setRuntimeError(null);
      })
      .catch((reason) => {
        setRuntimeError(
          typeof reason === "string" ? reason : String(reason)
        );
      });
  }, []);

  useEffect(() => {
    void refreshRuntimeStatus();
  }, [refreshRuntimeStatus]);

  useEffect(() => {
    let cancelled = false;

    onTaskProgress((progress) => {
      if (cancelled || progress.task_id) {
        return;
      }

      setProcessedCount(progress.processed_count);
      setRequestedCount(progress.requested_count);
      setItems((previous) => [...previous, progress.latest_item]);
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

  const updateField = useCallback(
    <K extends keyof FormState>(field: K, value: FormState[K]) => {
      setForm((previous) => ({ ...previous, [field]: value }));
    },
    []
  );

  const runtimeReady = Boolean(
    runtimeStatus?.openIdsReady &&
      runtimeStatus?.shopReady &&
      runtimeStatus?.provinceReady &&
      runtimeStatus?.fcReady
  );

  const hasValidInputs =
    form.sCourseId.trim() !== "" &&
    form.sManagerId.trim() !== "" &&
    form.fc.trim() !== "" &&
    parseShopcodesInput(form.shopcodesInput).length > 0;

  const canSubmit =
    runState !== "running" &&
    runtimeError === null &&
    runtimeStatus !== null &&
    runtimeReady &&
    hasValidInputs;

  const execute = useCallback(async () => {
    if (!canSubmit) {
      return;
    }

    const shopcodes = parseShopcodesInput(form.shopcodesInput);

    setRunState("running");
    setProcessedCount(0);
    setRequestedCount(shopcodes.length);
    setItems([]);
    setSummary(null);
    setError(null);
    setLinkedTaskId(null);
    setLinkedTaskProgress(null);
    setLinkedTaskResults(null);
    setLinkedTaskSyncError(null);

    try {
      const result = await runReadingTask({
        sCourseId: form.sCourseId.trim(),
        sManagerId: form.sManagerId.trim(),
        fc: form.fc.trim(),
        count: shopcodes.length,
        shopcodes,
        runDate: getTodayDate(),
      });

      setSummary(result);
      setItems(result.items);
      setProcessedCount(result.processed_count);
      setRequestedCount(result.requested_count);
      setRunState("completed");

      const archivedTaskId =
        result.archive_result?.status === "Archived"
          ? result.archive_result.task_id
          : null;

      if (archivedTaskId) {
        setLinkedTaskId(archivedTaskId);

        try {
          const [progress, taskResults] = await Promise.all([
            getDailyProgress(archivedTaskId, getTodayDate()),
            getTaskResults(archivedTaskId),
          ]);

          setLinkedTaskProgress(progress);
          setLinkedTaskResults(taskResults);
          setLinkedTaskSyncError(null);
        } catch (reason) {
          setLinkedTaskSyncError(
            `已归档到月度计划，但同步月度计划进度失败：${String(reason)}`,
          );
        }
      }
    } catch (reason: unknown) {
      const maybeCommandError = reason as CommandError;
      if (
        maybeCommandError &&
        typeof maybeCommandError === "object" &&
        "category" in maybeCommandError
      ) {
        setError(maybeCommandError);
      } else {
        setError({ category: "execution", message: String(reason) });
      }
      setRunState("error");
    }
  }, [canSubmit, form]);

  return {
    runtimeStatus,
    runtimeError,
    runtimeReady,
    form,
    updateField,
    runState,
    processedCount,
    requestedCount,
    items,
    summary,
    error,
    linkedTaskId,
    linkedTaskProgress,
    linkedTaskResults,
    linkedTaskSyncError,
    refreshRuntimeStatus,
    canSubmit,
    execute,
  };
}
