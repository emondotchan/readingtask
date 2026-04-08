import { ActivityIcon, ListTodoIcon, ZapIcon } from "lucide-react";

import ConfigStatus from "@/components/ConfigStatus";
import TaskForm from "@/components/TaskForm";
import MonthlyPlanManager from "@/components/MonthlyPlanManager";
import QuickRunResults from "@/components/QuickRunResults";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useTaskRunner } from "@/features/useTaskRunner";
import { useMonthlyRunner } from "@/features/useMonthlyRunner";
import { TaskStatusDialog } from "@/components/TaskStatusDialog";
import { useCallback, useState } from "react";
import { ModeToggle } from "@/components/ModeToggle";

export default function App() {
  const taskRunner = useTaskRunner();
  const monthlyRunner = useMonthlyRunner();
  const {
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
    canSubmit,
    execute,
  } = taskRunner;

  const [quickRunDialogOpen, setQuickRunDialogOpen] = useState(false);
  const handleRuntimeStatusChanged = useCallback(async () => {
    await Promise.all([
      taskRunner.refreshRuntimeStatus(),
      monthlyRunner.refreshRuntimeStatus(),
    ]);
  }, [monthlyRunner, taskRunner]);

  return (
    <main className="relative min-h-svh bg-background transition-colors duration-300">
      <div className="relative mx-auto flex min-h-svh max-w-370 flex-col gap-6 px-4 py-6 md:px-6 lg:px-8">
        {/* Header */}
        <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
          <div className="flex flex-col gap-1">
            <h1 className="text-3xl font-extrabold tracking-tight text-[#FF6F61] md:text-4xl">
              阅读任务工作台
            </h1>
            <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
              <ActivityIcon className="size-3.5 text-emerald-500" />
              <span className="uppercase tracking-widest text-[10px]">
                Core Engine Active
              </span>
            </div>
          </div>

          <div className="flex items-center gap-3 ml-auto">
            <ModeToggle />
          </div>
        </div>

        {/* Global Config Cards */}
        <ConfigStatus
          status={runtimeStatus}
          error={runtimeError}
          onRuntimeStatusChanged={handleRuntimeStatusChanged}
        />

        <Tabs defaultValue="monthly" className="flex flex-col gap-6">
          <TabsList className="grid h-10 w-100 grid-cols-2 self-start border-none bg-muted/70">
            <TabsTrigger
              value="monthly"
              className="gap-2 shadow-none data-[state=active]:bg-card"
            >
              <ListTodoIcon className="w-4 h-4" />
              月度计划
            </TabsTrigger>
            <TabsTrigger
              value="quick"
              className="gap-2 shadow-none data-[state=active]:bg-card"
            >
              <ZapIcon className="w-4 h-4" />
              快捷执行
            </TabsTrigger>
          </TabsList>

          <TabsContent value="monthly" className="mt-0 outline-none">
            <MonthlyPlanManager currentRun={monthlyRunner} />
          </TabsContent>

          <TabsContent value="quick" className="mt-0 outline-none">
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
              {/* Left Side: Form */}
              <div className="lg:col-span-5">
                <TaskForm
                  form={form}
                  updateField={updateField}
                  canSubmit={canSubmit}
                  running={runState === "running"}
                  runtimeReady={runtimeReady}
                  runtimeLoaded={runtimeStatus !== null}
                  runtimeConfigured={Boolean(runtimeStatus?.sqliteConfigured)}
                  runtimeError={runtimeError}
                  onSubmit={execute}
                />
              </div>

              {/* Right Side: Results */}
              <div className="lg:col-span-7 h-140">
                <QuickRunResults
                  runState={runState}
                  processedCount={processedCount}
                  requestedCount={requestedCount}
                  items={items}
                  summary={summary}
                  error={error}
                  linkedTaskId={linkedTaskId}
                  linkedTaskProgress={linkedTaskProgress}
                  linkedTaskResults={linkedTaskResults}
                  linkedTaskSyncError={linkedTaskSyncError}
                />
              </div>
            </div>
          </TabsContent>
        </Tabs>
      </div>

      {/* Keep Status Dialog for historical lookups if needed */}
      <TaskStatusDialog
        task={null}
        open={quickRunDialogOpen}
        onOpenChange={setQuickRunDialogOpen}
      />
    </main>
  );
}
