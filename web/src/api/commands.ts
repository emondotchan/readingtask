import type {
  RuntimeStatus,
  RunTaskInput,
  TaskRunSummary,
  MonthlyTask,
  MonthlyTaskPlanPreview,
  DailyTask,
  TaskItemResult,
  OpenIdRecord,
  CourseRecord,
  DailyTaskRunSnapshot,
  BatchRunDailyTasksResponse,
} from "../types";

const API_BASE = "/api";

async function readError(response: Response) {
  const contentType = response.headers.get("content-type") ?? "";

  if (contentType.includes("application/json")) {
    try {
      return await response.json();
    } catch {
      // Fall through to text parsing.
    }
  }

  const text = await response.text();
  return {
    category: "execution",
    message: text.trim() || response.statusText || "请求失败",
  };
}

async function requestJson<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: {
      "content-type": "application/json",
      ...(init.headers ?? {}),
    },
  });

  if (!response.ok) {
    throw await readError(response);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

async function requestVoid(
  path: string,
  init: RequestInit = {},
): Promise<void> {
  await requestJson<undefined>(path, init);
}

export async function getRuntimeStatus(): Promise<RuntimeStatus> {
  return requestJson<RuntimeStatus>("/runtime-status");
}

export async function setSqlitePath(sqlitePath: string): Promise<RuntimeStatus> {
  return requestJson<RuntimeStatus>("/sqlite-path", {
    method: "POST",
    body: JSON.stringify({ sqlitePath }),
  });
}

export async function runReadingTask(
  input: RunTaskInput,
): Promise<TaskRunSummary> {
  return requestJson<TaskRunSummary>("/run-reading-task", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function getOpenIds(): Promise<OpenIdRecord[]> {
  return requestJson<OpenIdRecord[]>("/open-ids");
}

export async function addOpenId(openId: OpenIdRecord): Promise<void> {
  await requestVoid("/open-ids", {
    method: "POST",
    body: JSON.stringify(openId),
  });
}

export async function deleteOpenId(openId: string): Promise<void> {
  await requestVoid(`/open-ids/${encodeURIComponent(openId)}`, {
    method: "DELETE",
  });
}

export interface ShopRecord {
  province: string;
  city: string;
  shop_code: string;
  shop_name: string;
  fc: string | null;
  shop_type: number;
}

export async function getShops(): Promise<ShopRecord[]> {
  return requestJson<ShopRecord[]>("/shops");
}

export async function importShops(shops: ShopRecord[]): Promise<number> {
  return requestJson<number>("/shops/import", {
    method: "POST",
    body: JSON.stringify(shops),
  });
}

export async function updateShopTypes(
  shopCodes: string[],
  shopType: number,
): Promise<number> {
  return requestJson<number>("/shops/shop-types", {
    method: "POST",
    body: JSON.stringify({ shopCodes, shopType }),
  });
}

export async function deleteAllShops(): Promise<void> {
  await requestVoid("/shops", {
    method: "DELETE",
  });
}

export interface FcRecord {
  name: string;
}

export interface UpsertFcInput {
  fc: FcRecord;
  previous_name?: string | null;
}

export interface UpsertCourseInput {
  course: CourseRecord;
  previous_month?: string | null;
  previous_course_id?: string | null;
  previous_task_type?: string | null;
}

export async function getFcs(): Promise<FcRecord[]> {
  return requestJson<FcRecord[]>("/fcs");
}

export async function addOrUpdateFc(input: UpsertFcInput): Promise<void> {
  await requestVoid("/fcs", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function deleteFc(name: string): Promise<void> {
  await requestVoid(`/fcs/${encodeURIComponent(name)}`, {
    method: "DELETE",
  });
}

export async function getCourses(): Promise<CourseRecord[]> {
  return requestJson<CourseRecord[]>("/courses");
}

export async function addOrUpdateCourse(input: UpsertCourseInput): Promise<void> {
  await requestVoid("/courses", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function deleteCourse(
  month: string,
  courseId: string,
  taskType: string,
): Promise<void> {
  const params = new URLSearchParams({
    month,
    course_id: courseId,
    task_type: taskType,
  });

  await requestVoid(`/courses?${params.toString()}`, {
    method: "DELETE",
  });
}

export async function getShopCount(
  fcName: string,
  taskType: string,
): Promise<number> {
  const params = new URLSearchParams({
    fcName,
    task_type: taskType,
  });

  return requestJson<number>(`/shop-count?${params.toString()}`);
}

export async function getMonthlyTasks(): Promise<MonthlyTask[]> {
  return requestJson<MonthlyTask[]>("/monthly-tasks");
}

export async function previewMonthlyTaskPlan(
  task: MonthlyTask,
): Promise<MonthlyTaskPlanPreview> {
  return requestJson<MonthlyTaskPlanPreview>("/monthly-tasks/preview", {
    method: "POST",
    body: JSON.stringify(task),
  });
}

export async function createMonthlyTask(
  task: MonthlyTask,
): Promise<MonthlyTaskPlanPreview> {
  return requestJson<MonthlyTaskPlanPreview>("/monthly-tasks", {
    method: "POST",
    body: JSON.stringify(task),
  });
}

export async function saveDailyTask(task: DailyTask): Promise<void> {
  await requestVoid(`/daily-tasks/${encodeURIComponent(task.task_id)}`, {
    method: "POST",
    body: JSON.stringify(task),
  });
}

export async function getTaskDailyTasks(taskId: string): Promise<DailyTask[]> {
  return requestJson<DailyTask[]>(
    `/daily-tasks/${encodeURIComponent(taskId)}/all`,
  );
}

export async function deleteMonthlyTask(id: string): Promise<void> {
  await requestVoid(`/monthly-tasks/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}

export async function getDailyTask(
  taskId: string,
  date: string,
): Promise<DailyTask | null> {
  const params = new URLSearchParams({ date });
  return requestJson<DailyTask | null>(
    `/daily-tasks/${encodeURIComponent(taskId)}?${params.toString()}`,
  );
}

export async function runDailyTask(
  taskId: string,
  date: string,
): Promise<TaskRunSummary> {
  const params = new URLSearchParams({ date });
  return requestJson<TaskRunSummary>(
    `/daily-tasks/${encodeURIComponent(taskId)}/run?${params.toString()}`,
    { method: "POST" },
  );
}

export async function startDailyTaskBatch(
  taskIds: string[],
  date: string,
): Promise<BatchRunDailyTasksResponse> {
  return requestJson<BatchRunDailyTasksResponse>("/daily-tasks/batch-run", {
    method: "POST",
    body: JSON.stringify({ taskIds, date }),
  });
}

export async function getDailyTaskRunStatuses(): Promise<DailyTaskRunSnapshot[]> {
  return requestJson<DailyTaskRunSnapshot[]>("/daily-tasks/run-status");
}

export async function pauseDailyTask(taskId: string): Promise<boolean> {
  return requestJson<boolean>(
    `/daily-tasks/${encodeURIComponent(taskId)}/pause`,
    { method: "POST" },
  );
}

export async function getTaskResults(taskId: string): Promise<TaskItemResult[]> {
  return requestJson<TaskItemResult[]>(
    `/tasks/${encodeURIComponent(taskId)}/results`,
  );
}
