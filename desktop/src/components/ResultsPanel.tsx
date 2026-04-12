import {
  AlertCircleIcon,
  CheckCircle2Icon,
  Clock3Icon,
  GaugeIcon,
  ListCollapseIcon,
  MapPinIcon,
  PlayCircleIcon,
  ServerCrashIcon,
  SparklesIcon,
} from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardAction,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";
import type { RunState } from "@/features/useTaskRunner";
import type { CommandError, TaskItemResult, TaskRunSummary } from "@/types";

interface Props {
  runState: RunState;
  processedCount: number;
  requestedCount: number;
  items: TaskItemResult[];
  summary: TaskRunSummary | null;
  error: CommandError | null;
}

const runStateMeta: Record<
  RunState,
  { label: string; badgeClassName: string }
> = {
  idle: {
    label: "等待执行",
    badgeClassName: "border-border bg-muted text-foreground",
  },
  running: {
    label: "执行中",
    badgeClassName: "border-sky-200 bg-sky-100 text-sky-700",
  },
  completed: {
    label: "已完成",
    badgeClassName: "border-emerald-200 bg-emerald-100 text-emerald-700",
  },
  error: {
    label: "执行异常",
    badgeClassName: "border-rose-200 bg-rose-100 text-rose-700",
  },
};

function formatTimestamp(value: string | undefined): string {
  if (!value) {
    return "—";
  }

  const seconds = Number(value);
  if (!Number.isFinite(seconds)) {
    return value;
  }

  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function outcomeLabel(outcome: TaskItemResult["outcome"]): string {
  switch (outcome) {
    case "Success":
      return "成功";
    case "RequestError":
      return "请求错误";
    case "ResponseReadError":
      return "响应读取错误";
  }
}

function errorCategoryLabel(category: string): string {
  switch (category) {
    case "validation":
      return "输入错误";
    case "config":
      return "配置错误";
    case "resource":
      return "资源错误";
    case "execution":
      return "执行错误";
    default:
      return category;
  }
}

export default function ResultsPanel({
  runState,
  processedCount,
  requestedCount,
  items,
  summary,
  error,
}: Props) {
  const runMeta = runStateMeta[runState];
  const progressValue = requestedCount > 0 ? (processedCount / requestedCount) * 100 : 0;
  const latestItem = items.length > 0 ? items[items.length - 1] : undefined;
  const displaySummary = {
    requested: summary?.requested_count ?? requestedCount,
    processed: summary?.processed_count ?? processedCount,
    success: summary?.success_count ?? items.filter((item) => item.outcome === "Success").length,
    failure:
      summary?.failure_count ??
      items.filter((item) => item.outcome !== "Success").length,
    startedAt: formatTimestamp(summary?.started_at),
    finishedAt: formatTimestamp(summary?.finished_at),
  };

  const summaryTiles = [
    {
      icon: GaugeIcon,
      label: "请求数",
      value: String(displaySummary.requested || 0),
    },
    {
      icon: ListCollapseIcon,
      label: "已处理",
      value: String(displaySummary.processed || 0),
    },
    {
      icon: CheckCircle2Icon,
      label: "成功",
      value: String(displaySummary.success || 0),
    },
    {
      icon: ServerCrashIcon,
      label: "失败",
      value: String(displaySummary.failure || 0),
    },
  ];

  return (
    <Card className="flex min-h-[560px] flex-1 flex-col shadow-sm">
      <CardHeader className="gap-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
          <div className="flex flex-col gap-2">
            <CardTitle>执行结果</CardTitle>
          </div>

          <CardAction className="static flex flex-wrap items-center gap-2 self-auto">
            <Badge variant="outline" className={runMeta.badgeClassName}>
              <SparklesIcon />
              {runMeta.label}
            </Badge>
            {requestedCount > 0 && (
              <Badge variant="outline" className="border-border bg-muted/60 text-muted-foreground">
                {processedCount} / {requestedCount} 完成
              </Badge>
            )}
          </CardAction>
        </div>
      </CardHeader>

      <CardContent className="flex flex-1 flex-col gap-5">
        {(runState === "running" || runState === "completed") && requestedCount > 0 && (
          <div className="rounded-xl border p-4">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="text-sm font-medium text-foreground">执行进度</p>
                <p className="text-sm text-muted-foreground">
                  {processedCount} / {requestedCount} 已完成
                </p>
              </div>
              <Badge variant="outline" className="border-border bg-muted/60 text-muted-foreground">
                {Math.round(progressValue)}%
              </Badge>
            </div>
            <Progress value={progressValue} className="h-2.5 rounded-full bg-muted" />
            {latestItem && (
              <div className="mt-4 flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
                <div className="inline-flex items-center gap-2 rounded-full border border-border bg-muted/50 px-3 py-1.5">
                  <MapPinIcon className="size-4 text-sky-600" />
                  最新门店：{latestItem.shop_code}
                </div>
                <div className="inline-flex items-center gap-2 rounded-full border border-border bg-muted/50 px-3 py-1.5">
                  <Clock3Icon className="size-4 text-violet-600" />
                  最新状态：{outcomeLabel(latestItem.outcome)}
                </div>
              </div>
            )}
          </div>
        )}

        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {summaryTiles.map(({ icon: Icon, label, value }) => (
            <div
              key={label}
              className="rounded-xl border p-4 shadow-sm"
            >
              <div className="mb-3 inline-flex size-10 items-center justify-center rounded-md bg-muted text-foreground">
                <Icon className="size-4" />
              </div>
              <div className="flex flex-col gap-1">
                <p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
                  {label}
                </p>
                <p className="text-2xl font-semibold tracking-tight text-foreground">
                  {value}
                </p>
              </div>
            </div>
          ))}
        </div>

        {(summary?.started_at || summary?.finished_at) && (
          <div className="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
            <div className="inline-flex items-center gap-2 rounded-full border border-border bg-muted/50 px-3 py-1.5">
              <Clock3Icon className="size-4 text-muted-foreground" />
              开始：{displaySummary.startedAt}
            </div>
            <div className="inline-flex items-center gap-2 rounded-full border border-border bg-muted/50 px-3 py-1.5">
              <Clock3Icon className="size-4 text-muted-foreground" />
              结束：{displaySummary.finishedAt}
            </div>
          </div>
        )}

        {error && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>{errorCategoryLabel(error.category)}</AlertTitle>
            <AlertDescription>{error.message}</AlertDescription>
          </Alert>
        )}

        <Separator />

        <div className="min-h-[320px] flex-1 rounded-xl border p-2">
          {items.length === 0 ? (
            <Empty className="min-h-[300px] border-0 bg-transparent text-muted-foreground">
              <EmptyHeader>
                <EmptyMedia variant="icon" className="bg-muted text-sky-700 shadow-sm dark:text-sky-400">
                  <PlayCircleIcon />
                </EmptyMedia>
                <EmptyTitle>等待首轮任务执行</EmptyTitle>
              </EmptyHeader>
            </Empty>
          ) : (
            <ScrollArea className="h-[420px] rounded-xl">
              <Table>
                <TableHeader>
                  <TableRow className="bg-muted/50 hover:bg-muted/50">
                    <TableHead className="sticky top-0 z-10 bg-muted">#</TableHead>
                    <TableHead className="sticky top-0 z-10 bg-muted">OpenID</TableHead>
                    <TableHead className="sticky top-0 z-10 bg-muted">ShopCode</TableHead>
                    <TableHead className="sticky top-0 z-10 bg-muted">地区</TableHead>
                    <TableHead className="sticky top-0 z-10 bg-muted">HTTP 状态</TableHead>
                    <TableHead className="sticky top-0 z-10 bg-muted">结果 / 错误</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {items.map((item) => {
                    const isSuccess = item.outcome === "Success";

                    return (
                      <TableRow
                        key={`${item.index}-${item.shop_code}-${item.open_id}`}
                        className={cn(
                          isSuccess
                            ? "bg-emerald-50/35 hover:bg-emerald-50/55 dark:bg-emerald-950/10 dark:hover:bg-emerald-950/20"
                            : "bg-rose-50/35 hover:bg-rose-50/55 dark:bg-rose-950/10 dark:hover:bg-rose-950/20"
                        )}
                      >
                        <TableCell className="font-medium text-foreground">
                          {item.index}
                        </TableCell>
                        <TableCell className="max-w-40 truncate font-mono text-xs text-muted-foreground">
                          {item.open_id}
                        </TableCell>
                        <TableCell className="font-mono text-xs text-foreground">
                          {item.shop_code}
                        </TableCell>
                        <TableCell className="whitespace-normal text-foreground">
                          {item.province}
                          {item.city ? ` / ${item.city}` : ""}
                        </TableCell>
                        <TableCell>
                          <Badge variant="outline" className="border-border text-foreground">
                            {item.http_status ?? "—"}
                          </Badge>
                        </TableCell>
                        <TableCell className="max-w-[22rem] whitespace-normal">
                          <div className="flex flex-col gap-2">
                            <Badge
                              variant="outline"
                              className={cn(
                                "w-fit",
                                isSuccess
                                  ? "text-emerald-700 dark:text-emerald-300"
                                  : "text-rose-700 dark:text-rose-300"
                              )}
                            >
                              {isSuccess ? <CheckCircle2Icon /> : <AlertCircleIcon />}
                              {outcomeLabel(item.outcome)}
                            </Badge>
                            <p className="text-sm leading-6 text-muted-foreground">
                              {isSuccess
                                ? item.response_text || "请求已成功完成。"
                                : item.rtn_msg || item.response_text || "执行失败，未返回更多信息。"}
                            </p>
                          </div>
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </ScrollArea>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
