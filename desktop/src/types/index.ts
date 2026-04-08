// --- Tauri command types ---
// RuntimeStatus and RunTaskInput use camelCase (serde rename_all = "camelCase")
// TaskRunSummary, TaskItemResult, TaskProgress use snake_case (no serde rename)
// CommandError uses camelCase

export interface RuntimeStatus {
  sqlitePath: string | null;
  sqliteConfigured: boolean;
  openIdsReady: boolean;
  shopReady: boolean;
  provinceReady: boolean;
  fcReady: boolean;
}

export interface RunTaskInput {
  sCourseId: string;
  sManagerId: string;
  fc: string;
  count: number;
  shopcodes: string[];
  runDate: string;
}

export interface TaskRunSummary {
  requested_count: number;
  processed_count: number;
  success_count: number;
  failure_count: number;
  started_at: string;
  finished_at: string;
  items: TaskItemResult[];
  archive_result: QuickRunArchiveResult | null;
}

export interface QuickRunArchiveResult {
  status: "Archived" | "NoMatchingTask" | "DuplicateTasks";
  task_id: string | null;
  message: string;
}

export interface TaskItemResult {
  index: number;
  executed_date?: string | null;
  submit_err?: number | null;
  rtn_msg?: string | null;
  read_id?: string | null;
  open_id: string;
  shop_code: string;
  province: string;
  city: string;
  http_status: number | null;
  response_text: string | null;
  error_message: string | null;
  outcome: "Success" | "RequestError" | "ResponseReadError";
}

export interface TaskProgress {
  task_id?: string | null;
  processed_count: number;
  requested_count: number;
  latest_item: TaskItemResult;
}

export interface CommandError {
  category: string;
  message: string;
}

export interface OpenIdRecord {
  manager_id: string;
  open_id: string;
}

export interface MonthlyTask {
  id: string;
  fc_name: string;
  s_manager_id: string;
  s_course_id: string;
  task_type: "Avene" | "Klorane";
  total_target: number;
  target_days: number;
  created_at: string;
  shopcodes: string[];
}

export interface DailyProgress {
  task_id: string;
  date: string;
  target_count: number;
  completed_count: number;
  is_locked: boolean;
  shopcodes: string[];
}

export interface MonthlyTaskPlanPreview {
  eligible_shop_count: number;
  total_target: number;
  target_days: number;
  daily_plans: DailyProgress[];
}
