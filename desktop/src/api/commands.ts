import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  RuntimeStatus,
  RunTaskInput,
  TaskRunSummary,
  TaskProgress,
  MonthlyTask,
  MonthlyTaskPlanPreview,
  DailyProgress,
  TaskItemResult,
  OpenIdRecord,
} from "../types";

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  return invoke<RuntimeStatus>("get_runtime_status");
}

export async function setSqlitePath(sqlitePath: string): Promise<RuntimeStatus> {
  return invoke<RuntimeStatus>("set_sqlite_path", { sqlitePath });
}

export async function runReadingTask(
  input: RunTaskInput
): Promise<TaskRunSummary> {
  return invoke<TaskRunSummary>("run_reading_task", { request: input });
}

export async function onTaskProgress(
  callback: (progress: TaskProgress) => void
): Promise<UnlistenFn> {
  return listen<TaskProgress>("reading-task://progress", (event) => {
    callback(event.payload);
  });
}

export async function getOpenIds(): Promise<OpenIdRecord[]> {
  return invoke<OpenIdRecord[]>("get_open_ids");
}

export async function addOpenId(openId: OpenIdRecord): Promise<void> {
  return invoke<void>("add_open_id", { openId });
}

export async function deleteOpenId(openId: string): Promise<void> {
  return invoke<void>("delete_open_id", { openId });
}

export async function importOpenIdsCsv(csvText: string): Promise<number> {
  return invoke<number>("import_open_ids_csv", { csvText });
}

export interface ShopRecord {
  province: string;
  city: string;
  shop_code: string;
  fc: string | null;
  shop_type: number;
}

export async function getShops(): Promise<ShopRecord[]> {
  return invoke<ShopRecord[]>("get_shops");
}

export async function addOrUpdateShop(shop: ShopRecord): Promise<void> {
  return invoke<void>("add_or_update_shop", { shop });
}

export async function deleteShop(shopCode: string): Promise<void> {
  return invoke<void>("delete_shop", { shopCode });
}

export interface FcRecord {
  name: string;
  manager_id: string;
}

export async function getFcs(): Promise<FcRecord[]> {
  return invoke<FcRecord[]>("get_fcs");
}

export async function addOrUpdateFc(fc: FcRecord): Promise<void> {
  return invoke<void>("add_or_update_fc", { fc });
}

export async function deleteFc(name: string): Promise<void> {
  return invoke<void>("delete_fc", { name });
}

export async function getShopCount(fcName: string, taskType: string): Promise<number> {
  return invoke<number>("get_shop_count", { fcName, taskType });
}

export async function getMonthlyTasks(): Promise<MonthlyTask[]> {
  return invoke<MonthlyTask[]>("get_monthly_tasks");
}

export async function previewMonthlyTaskPlan(
  task: MonthlyTask,
): Promise<MonthlyTaskPlanPreview> {
  return invoke<MonthlyTaskPlanPreview>("preview_monthly_task_plan", { task });
}

export async function createMonthlyTask(
  task: MonthlyTask,
): Promise<MonthlyTaskPlanPreview> {
  return invoke<MonthlyTaskPlanPreview>("create_monthly_task", { task });
}

export async function deleteMonthlyTask(id: string): Promise<void> {
  return invoke<void>("delete_monthly_task", { id });
}

export async function getDailyProgress(taskId: string, date: string): Promise<DailyProgress | null> {
  return invoke<DailyProgress | null>("get_daily_progress", { taskId, date });
}

export async function runDailyTask(taskId: string, date: string): Promise<TaskRunSummary> {
  return invoke<TaskRunSummary>("run_daily_task", { taskId, date });
}

export async function pauseDailyTask(taskId: string): Promise<boolean> {
  return invoke<boolean>("pause_daily_task", { taskId });
}

export async function getTaskResults(taskId: string): Promise<TaskItemResult[]> {
  return invoke<TaskItemResult[]>("get_task_results", { taskId });
}
