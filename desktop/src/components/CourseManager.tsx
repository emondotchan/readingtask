import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  PencilIcon,
  PlusIcon,
  Trash2Icon,
  BookOpenIcon,
} from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Field, FieldLabel } from "@/components/ui/field";
import {
  Empty,
  EmptyHeader,
  EmptyTitle,
  EmptyMedia,
} from "@/components/ui/empty";
import {
  addOrUpdateCourse,
  deleteCourse,
  getCourses,
  type UpsertCourseInput,
} from "@/api/commands";
import type { CourseRecord } from "@/types";
import { cn } from "@/lib/utils";

const PAGE_SIZE = 12;
const CURRENT_MONTH = new Date().toISOString().slice(0, 7);
const EMPTY_FORM: CourseRecord = {
  month: CURRENT_MONTH,
  course_id: "",
  task_type: "Avene",
};

function taskTypeBadgeClassName(taskType: CourseRecord["task_type"]) {
  return taskType === "Avene"
    ? "border-primary/20 bg-primary/10 text-primary dark:border-primary/30 dark:bg-primary/15"
    : "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-950/20 dark:text-emerald-300";
}

export function CourseManager({
  open,
  onOpenChange,
  onCoursesChanged,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCoursesChanged?: () => Promise<void> | void;
}) {
  const [courses, setCourses] = useState<CourseRecord[]>([]);
  const [formData, setFormData] = useState<CourseRecord>(EMPTY_FORM);
  const [editingKey, setEditingKey] = useState<{
    month: string;
    course_id: string;
    task_type: CourseRecord["task_type"];
  } | null>(null);
  const [monthFilter, setMonthFilter] = useState("all");
  const [typeFilter, setTypeFilter] = useState<"all" | "Avene" | "Klorane">(
    "all",
  );
  const [currentPage, setCurrentPage] = useState(1);
  const [feedback, setFeedback] = useState<string | null>(null);

  const resetForm = () => {
    setFormData(EMPTY_FORM);
    setEditingKey(null);
  };

  const loadCourses = async () => {
    try {
      setCourses(await getCourses());
    } catch (error) {
      console.error(error);
      setFeedback(error instanceof Error ? error.message : String(error));
    }
  };

  useEffect(() => {
    if (!open) {
      return;
    }

    void loadCourses();
    setMonthFilter("all");
    setTypeFilter("all");
    setCurrentPage(1);
    setFeedback(null);
    resetForm();
  }, [open]);

  const monthOptions = useMemo(
    () =>
      Array.from(new Set(courses.map((course) => course.month))).sort((a, b) =>
        b.localeCompare(a),
      ),
    [courses],
  );

  const filteredCourses = useMemo(
    () =>
      courses.filter((course) => {
        const matchesMonth =
          monthFilter === "all" || course.month === monthFilter;
        const matchesType =
          typeFilter === "all" || course.task_type === typeFilter;
        return matchesMonth && matchesType;
      }),
    [courses, monthFilter, typeFilter],
  );

  const totalPages = Math.max(1, Math.ceil(filteredCourses.length / PAGE_SIZE));

  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [currentPage, totalPages]);

  const currentCourses = filteredCourses.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE,
  );

  const handleSave = async (event?: FormEvent) => {
    event?.preventDefault();

    if (!formData.month || !formData.course_id.trim()) {
      setFeedback("请填写月份和课程 ID。");
      return;
    }

    try {
      const input: UpsertCourseInput = {
        course: {
          month: formData.month,
          course_id: formData.course_id.trim(),
          task_type: formData.task_type,
        },
        previous_month: editingKey?.month,
        previous_course_id: editingKey?.course_id,
        previous_task_type: editingKey?.task_type,
      };

      await addOrUpdateCourse(input);
      await loadCourses();
      await onCoursesChanged?.();
      setFeedback(editingKey ? "课程已更新" : "课程已保存");
      resetForm();
    } catch (error) {
      console.error(error);
      setFeedback(error instanceof Error ? error.message : String(error));
    }
  };

  const handleEdit = (course: CourseRecord) => {
    setFormData(course);
    setEditingKey(course);
    setFeedback(null);
  };

  const handleDelete = async (course: CourseRecord) => {
    try {
      await deleteCourse(course.month, course.course_id, course.task_type);
      if (
        editingKey?.month === course.month &&
        editingKey.course_id === course.course_id &&
        editingKey.task_type === course.task_type
      ) {
        resetForm();
      }
      await loadCourses();
      await onCoursesChanged?.();
    } catch (error) {
      console.error(error);
      setFeedback(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-4xl w-[95vw] max-h-[90vh] overflow-hidden gap-0 flex flex-col p-0">
        <DialogHeader className="px-6 py-4 border-b border-border shrink-0">
          <DialogTitle className="flex items-center gap-2 text-xl">
            <BookOpenIcon className="size-5 text-muted-foreground" />
            课程管理
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto p-6 space-y-6 bg-muted/10">
          {feedback && (
            <Alert variant="default" className="bg-background">
              <AlertDescription>{feedback}</AlertDescription>
            </Alert>
          )}

          <div className="rounded-xl border border-border bg-card p-4 shadow-sm transition-all hover:border-primary/20">
            <form
              className="grid grid-cols-1 items-end gap-4 md:grid-cols-[1fr_1.5fr_1fr_auto]"
              onSubmit={handleSave}
            >
              <Field>
                <FieldLabel>月份</FieldLabel>
                <Input
                  type="month"
                  value={formData.month}
                  onChange={(event) =>
                    setFormData((previous) => ({
                      ...previous,
                      month: event.target.value,
                    }))
                  }
                />
              </Field>
              <Field>
                <FieldLabel>课程 ID</FieldLabel>
                <Input
                  placeholder="输入课程 ID"
                  value={formData.course_id}
                  onChange={(event) =>
                    setFormData((previous) => ({
                      ...previous,
                      course_id: event.target.value,
                    }))
                  }
                />
              </Field>
              <Field>
                <FieldLabel>任务类型</FieldLabel>
                <Select
                  value={formData.task_type}
                  onValueChange={(value) =>
                    setFormData((previous) => ({
                      ...previous,
                      task_type: value as CourseRecord["task_type"],
                    }))
                  }
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择类型" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="Avene">Avene</SelectItem>
                    <SelectItem value="Klorane">Klorane</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
              <div className="flex gap-2">
                <Button type="submit" className="w-[100px]">
                  {editingKey ? (
                    <PencilIcon className="mr-2 h-4 w-4" />
                  ) : (
                    <PlusIcon className="mr-2 h-4 w-4" />
                  )}
                  {editingKey ? "更新" : "添加"}
                </Button>
                {editingKey && (
                  <Button type="button" variant="outline" onClick={resetForm}>
                    取消
                  </Button>
                )}
              </div>
            </form>
          </div>

          <div className="flex flex-col gap-4">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
              <div className="flex items-center gap-2">
                <h3 className="text-sm font-semibold text-foreground">
                  课程列表
                </h3>
                <Badge variant="secondary" className="font-mono">
                  {filteredCourses.length}
                </Badge>
              </div>
              <div className="flex flex-1 sm:flex-none items-center gap-3">
                <Select
                  value={monthFilter}
                  onValueChange={(value) => {
                    setMonthFilter(value);
                    setCurrentPage(1);
                  }}
                >
                  <SelectTrigger className="w-[140px] bg-card">
                    <SelectValue placeholder="筛选月份" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部月份</SelectItem>
                    {monthOptions.map((month) => (
                      <SelectItem key={month} value={month}>
                        {month}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select
                  value={typeFilter}
                  onValueChange={(value) => {
                    setTypeFilter(value as "all" | "Avene" | "Klorane");
                    setCurrentPage(1);
                  }}
                >
                  <SelectTrigger className="w-[120px] bg-card">
                    <SelectValue placeholder="筛选类型" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部类型</SelectItem>
                    <SelectItem value="Avene">Avene</SelectItem>
                    <SelectItem value="Klorane">Klorane</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
              <ScrollArea className="h-[380px]">
                <Table>
                  <TableHeader>
                    <TableRow className="bg-muted/50 hover:bg-muted/50">
                      <TableHead className="w-[140px] font-semibold text-muted-foreground uppercase tracking-wider text-[11px] sticky top-0 bg-muted/90 backdrop-blur z-10">
                        月份
                      </TableHead>
                      <TableHead className="font-semibold text-muted-foreground uppercase tracking-wider text-[11px] sticky top-0 bg-muted/90 backdrop-blur z-10">
                        课程 ID
                      </TableHead>
                      <TableHead className="w-[140px] font-semibold text-muted-foreground uppercase tracking-wider text-[11px] sticky top-0 bg-muted/90 backdrop-blur z-10">
                        任务类型
                      </TableHead>
                      <TableHead className="w-[120px] text-center font-semibold text-muted-foreground uppercase tracking-wider text-[11px] sticky top-0 bg-muted/90 backdrop-blur z-10">
                        操作
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {currentCourses.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={4} className="h-[300px]">
                          <Empty className="border-0 shadow-none">
                            <EmptyHeader>
                              <EmptyMedia
                                variant="icon"
                                className="bg-muted text-muted-foreground"
                              >
                                <BookOpenIcon className="h-8 w-8 opacity-50" />
                              </EmptyMedia>
                              <EmptyTitle>未找到匹配的课程</EmptyTitle>
                            </EmptyHeader>
                          </Empty>
                        </TableCell>
                      </TableRow>
                    ) : (
                      currentCourses.map((course) => (
                        <TableRow
                          key={`${course.month}:${course.task_type}:${course.course_id}`}
                          className="group hover:bg-muted/40 transition-colors"
                        >
                          <TableCell className="font-mono text-sm text-muted-foreground">
                            {course.month}
                          </TableCell>
                          <TableCell className="font-mono text-sm font-medium">
                            {course.course_id}
                          </TableCell>
                          <TableCell>
                            <Badge
                              variant="outline"
                              className={cn(
                                "font-medium",
                                taskTypeBadgeClassName(course.task_type),
                              )}
                            >
                              {course.task_type}
                            </Badge>
                          </TableCell>
                          <TableCell className="text-center">
                            <div className="flex items-center justify-center gap-1 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto">
                              <Button
                                variant="ghost"
                                size="icon"
                                className="h-8 w-8 text-muted-foreground hover:text-foreground"
                                onClick={() => handleEdit(course)}
                                title="编辑"
                              >
                                <PencilIcon className="h-4 w-4" />
                              </Button>
                              <Button
                                variant="ghost"
                                size="icon"
                                className="h-8 w-8 text-rose-500 hover:text-rose-600 hover:bg-rose-50 dark:hover:bg-rose-950/50"
                                onClick={() => handleDelete(course)}
                                title="删除"
                              >
                                <Trash2Icon className="h-4 w-4" />
                              </Button>
                            </div>
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </ScrollArea>
            </div>

            {filteredCourses.length > 0 && (
              <div className="flex items-center justify-between pb-2">
                <span className="text-xs text-muted-foreground">
                  第 {currentPage} / {totalPages} 页，共{" "}
                  {filteredCourses.length} 条记录
                </span>
                <div className="flex items-center gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8 w-8 p-0"
                    onClick={() =>
                      setCurrentPage((page) => Math.max(1, page - 1))
                    }
                    disabled={currentPage === 1}
                  >
                    <ChevronLeftIcon className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8 w-8 p-0"
                    onClick={() =>
                      setCurrentPage((page) => Math.min(totalPages, page + 1))
                    }
                    disabled={currentPage === totalPages}
                  >
                    <ChevronRightIcon className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
