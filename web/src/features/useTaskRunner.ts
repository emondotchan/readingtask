import { useCallback, useEffect, useState } from "react";

import {
  getRuntimeStatus,
  runReadingTask,
} from "@/api/commands";
import { parseShopcodesInput } from "@/lib/utils";
import type {
  CommandError,
  RuntimeStatus,
  TaskItemResult,
  TaskRunSummary,
} from "@/types";

export type RunState = "idle" | "running" | "completed" | "error";

export interface FormState {
  readingUrl: string;
  fc: string;
  shopcodesInput: string;
}

const defaultForm: FormState = {
  readingUrl: "",
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
    form.readingUrl.trim() !== "" &&
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

    try {
      const result = await runReadingTask({
        sCourseId: "",
        sManagerId: "",
        readingUrl: form.readingUrl.trim(),
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
    refreshRuntimeStatus,
    canSubmit,
    execute,
  };
}
