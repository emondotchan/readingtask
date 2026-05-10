import { useEffect, useMemo, useState } from "react";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  KeyRoundIcon,
  PencilIcon,
  PlusIcon,
  SearchIcon,
  Trash2Icon,
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
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { addOpenId, deleteOpenId, getFcs, getOpenIds, type FcRecord } from "@/api/commands";
import type { OpenIdRecord } from "@/types";
import { getErrorMessage } from "@/lib/utils";

const PAGE_SIZE = 50;

function parseBatchOpenIds(raw: string) {
  const uniqueOpenIds: string[] = [];
  const seenOpenIds = new Set<string>();

  for (const [index, line] of raw.split(/\r?\n/).entries()) {
    const openId = (index === 0 ? line.trimStart().replace(/^\uFEFF/, "") : line).trim();
    if (!openId || seenOpenIds.has(openId)) {
      continue;
    }
    seenOpenIds.add(openId);
    uniqueOpenIds.push(openId);
  }

  return uniqueOpenIds;
}

export function OpenIdManager({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [openIds, setOpenIds] = useState<OpenIdRecord[]>([]);
  const [fcs, setFcs] = useState<FcRecord[]>([]);
  const [newFcName, setNewFcName] = useState("");
  const [newOpenId, setNewOpenId] = useState("");
  const [editingOpenId, setEditingOpenId] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedOpenIds, setSelectedOpenIds] = useState<Set<string>>(
    new Set(),
  );
  const [batchDialogOpen, setBatchDialogOpen] = useState(false);
  const [batchFcName, setBatchFcName] = useState("");
  const [batchOpenIdsText, setBatchOpenIdsText] = useState("");
  const [batchSaving, setBatchSaving] = useState(false);

  const loadData = async () => {
    try {
      const [openIdData, fcData] = await Promise.all([getOpenIds(), getFcs()]);
      setOpenIds(openIdData);
      setFcs(fcData);
      setSelectedOpenIds((previous) => {
        const availableOpenIds = new Set(
          openIdData.map((record) => record.open_id),
        );
        return new Set(
          [...previous].filter((openId) => availableOpenIds.has(openId)),
        );
      });
    } catch (error) {
      console.error(error);
    }
  };

  useEffect(() => {
    if (!open) {
      return;
    }

    void loadData();
    setCurrentPage(1);
    setFeedback(null);
    setEditingOpenId(null);
    setSearchTerm("");
    setSelectedOpenIds(new Set());
    setBatchDialogOpen(false);
    setBatchFcName("");
    setBatchOpenIdsText("");
    setBatchSaving(false);
  }, [open]);

  const filteredOpenIds = useMemo(() => {
    const term = searchTerm.toLowerCase().trim();
    if (!term) {
      return openIds;
    }

    return openIds.filter(
      (record) =>
        record.open_id.toLowerCase().includes(term) ||
        record.fc_name.toLowerCase().includes(term),
    );
  }, [openIds, searchTerm]);

  const totalPages = Math.max(1, Math.ceil(filteredOpenIds.length / PAGE_SIZE));

  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [currentPage, totalPages]);

  const resetForm = () => {
    setNewFcName("");
    setNewOpenId("");
    setEditingOpenId(null);
  };

  const resetBatchForm = () => {
    setBatchFcName("");
    setBatchOpenIdsText("");
    setBatchSaving(false);
  };

  const handleSave = async () => {
    if (!newFcName.trim() || !newOpenId.trim()) return;

    const trimmedFcName = newFcName.trim();
    const trimmedOpenId = newOpenId.trim();

    try {
      if (editingOpenId && editingOpenId !== trimmedOpenId) {
        await deleteOpenId(editingOpenId);
      }

      await addOpenId({
        fc_name: trimmedFcName,
        open_id: trimmedOpenId,
      });

      resetForm();
      setFeedback(editingOpenId ? "OpenID 已更新" : "OpenID 已保存");
      await loadData();
    } catch (error) {
      console.error(error);
      setFeedback(getErrorMessage(error));
    }
  };

  const handleEdit = (record: OpenIdRecord) => {
    setNewFcName(record.fc_name);
    setNewOpenId(record.open_id);
    setEditingOpenId(record.open_id);
    setFeedback(null);
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteOpenId(id);
      if (editingOpenId === id) {
        resetForm();
      }
      setSelectedOpenIds((previous) => {
        const next = new Set(previous);
        next.delete(id);
        return next;
      });
      await loadData();
    } catch (error) {
      console.error(error);
      setFeedback(getErrorMessage(error));
    }
  };

  const handleDeleteSelected = async () => {
    if (selectedOpenIds.size === 0) {
      return;
    }

    try {
      const selectedIds = [...selectedOpenIds];
      await Promise.all(selectedIds.map((openId) => deleteOpenId(openId)));
      if (editingOpenId && selectedOpenIds.has(editingOpenId)) {
        resetForm();
      }
      setSelectedOpenIds(new Set());
      setFeedback(`已删除 ${selectedIds.length} 条 OpenID 记录`);
      await loadData();
    } catch (error) {
      console.error(error);
      setFeedback(getErrorMessage(error));
    }
  };

  const handleBatchCreate = async () => {
    if (!batchFcName.trim()) {
      setFeedback("请先选择 FC");
      return;
    }

    const parsedOpenIds = parseBatchOpenIds(batchOpenIdsText);
    if (parsedOpenIds.length === 0) {
      setFeedback("请输入 OpenID，每行一个");
      return;
    }

    const existingOpenIds = new Set(openIds.map((record) => record.open_id));
    const uniqueNewOpenIds = parsedOpenIds.filter(
      (openId) => !existingOpenIds.has(openId),
    );
    const duplicateCount = parsedOpenIds.length - uniqueNewOpenIds.length;

    if (uniqueNewOpenIds.length === 0) {
      setFeedback(`没有可新增的 OpenID，已忽略 ${duplicateCount} 条重复记录`);
      return;
    }

    setBatchSaving(true);
    try {
      await Promise.all(
        uniqueNewOpenIds.map((open_id) =>
          addOpenId({
            fc_name: batchFcName,
            open_id,
          }),
        ),
      );

      resetBatchForm();
      setBatchDialogOpen(false);
      setFeedback(
        duplicateCount > 0
          ? `成功新增 ${uniqueNewOpenIds.length} 条 OpenID，已忽略 ${duplicateCount} 条重复记录`
          : `成功新增 ${uniqueNewOpenIds.length} 条 OpenID`,
      );
      await loadData();
    } catch (error) {
      console.error(error);
      setFeedback(getErrorMessage(error));
    } finally {
      setBatchSaving(false);
    }
  };

  const currentIds = filteredOpenIds.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE,
  );
  const currentPageOpenIds = currentIds.map((record) => record.open_id);
  const selectedCount = selectedOpenIds.size;
  const allCurrentPageSelected =
    currentPageOpenIds.length > 0 &&
    currentPageOpenIds.every((openId) => selectedOpenIds.has(openId));

  const toggleOpenIdSelection = (openId: string, checked: boolean) => {
    setSelectedOpenIds((previous) => {
      const next = new Set(previous);
      if (checked) {
        next.add(openId);
      } else {
        next.delete(openId);
      }
      return next;
    });
  };

  const toggleCurrentPageSelection = (checked: boolean) => {
    setSelectedOpenIds((previous) => {
      const next = new Set(previous);
      for (const openId of currentPageOpenIds) {
        if (checked) {
          next.add(openId);
        } else {
          next.delete(openId);
        }
      }
      return next;
    });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-5xl w-[95vw] max-h-[90vh] overflow-hidden gap-0 flex flex-col p-0">
        <DialogHeader className="px-6 py-4 border-b border-border shrink-0">
          <DialogTitle className="flex items-center gap-2 text-xl">
            <KeyRoundIcon className="size-5 text-muted-foreground" />
            OpenID 管理
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto p-6 space-y-6 bg-muted/10">
          {feedback && (
            <Alert className="bg-background">
              <AlertDescription>{feedback}</AlertDescription>
            </Alert>
          )}

          <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-[180px_1fr_auto_auto_auto]">
              <Select value={newFcName} onValueChange={setNewFcName}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="选择 FC" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {fcs.map((fc) => (
                      <SelectItem key={fc.name} value={fc.name}>
                        {fc.name}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
              <Input
                placeholder="OpenID"
                value={newOpenId}
                onChange={(event) => setNewOpenId(event.target.value)}
                onKeyDown={(event) =>
                  event.key === "Enter" && void handleSave()
                }
              />
              <Button onClick={() => void handleSave()} className="w-[110px]">
                {editingOpenId ? (
                  <PencilIcon className="mr-2 h-4 w-4" />
                ) : (
                  <PlusIcon className="mr-2 h-4 w-4" />
                )}
                {editingOpenId ? "更新" : "新增"}
              </Button>
              <Button
                variant="outline"
                onClick={resetForm}
                disabled={!newFcName && !newOpenId && !editingOpenId}
              >
                取消
              </Button>
              <Button
                variant="outline"
                onClick={() => {
                  setBatchDialogOpen(true);
                  setFeedback(null);
                }}
              >
                <PlusIcon className="mr-2 h-4 w-4" />
                批量新建
              </Button>
            </div>
          </div>

          <div className="space-y-4">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div className="flex flex-wrap items-center gap-2">
                <h3 className="text-sm font-semibold text-foreground">
                  OpenID 列表
                </h3>
                <Badge variant="secondary" className="font-mono">
                  {filteredOpenIds.length}
                </Badge>
                <Badge variant="outline" className="font-mono">
                  已选 {selectedCount}
                </Badge>
              </div>
              <div className="flex w-full flex-col gap-3 sm:flex-row lg:w-auto lg:items-center">
                <div className="relative w-full sm:w-[280px]">
                  <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                  <Input
                    placeholder="搜索 FC / OpenID"
                    className="pl-9 bg-card"
                    value={searchTerm}
                    onChange={(event) => {
                      setSearchTerm(event.target.value);
                      setCurrentPage(1);
                      setSelectedOpenIds(new Set());
                    }}
                  />
                </div>
                <Button
                  variant="outline"
                  onClick={() =>
                    toggleCurrentPageSelection(!allCurrentPageSelected)
                  }
                  disabled={currentIds.length === 0}
                >
                  {allCurrentPageSelected ? "取消全选当前页" : "全选当前页"}
                </Button>
                <Button
                  variant="destructive"
                  onClick={() => void handleDeleteSelected()}
                  disabled={selectedCount === 0}
                >
                  <Trash2Icon className="mr-2 h-4 w-4" />
                  删除选中
                </Button>
              </div>
            </div>

            <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
              <ScrollArea className="h-[400px]">
                <Table>
                  <TableHeader>
                    <TableRow className="bg-muted/50 hover:bg-muted/50">
                      <TableHead className="w-12 sticky top-0 bg-muted/90 text-center">
                        <input
                          type="checkbox"
                          className="size-4 accent-primary"
                          aria-label="全选当前页 OpenID"
                          checked={allCurrentPageSelected}
                          onChange={(event) =>
                            toggleCurrentPageSelection(event.target.checked)
                          }
                        />
                      </TableHead>
                      <TableHead className="w-[160px] sticky top-0 bg-muted/90">
                        FC
                      </TableHead>
                      <TableHead className="sticky top-0 bg-muted/90">
                        OpenID
                      </TableHead>
                      <TableHead className="w-[120px] sticky top-0 bg-muted/90 text-center">
                        操作
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {currentIds.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={4}
                          className="h-32 text-center text-muted-foreground"
                        >
                          暂无记录
                        </TableCell>
                      </TableRow>
                    ) : (
                      currentIds.map((record) => (
                        <TableRow
                          key={record.open_id}
                          className="group hover:bg-muted/40"
                        >
                          <TableCell className="text-center">
                            <input
                              type="checkbox"
                              className="size-4 accent-primary"
                              aria-label={`选择 OpenID ${record.open_id}`}
                              checked={selectedOpenIds.has(record.open_id)}
                              onChange={(event) =>
                                toggleOpenIdSelection(
                                  record.open_id,
                                  event.target.checked,
                                )
                              }
                            />
                          </TableCell>
                          <TableCell className="font-medium">
                            {record.fc_name || "-"}
                          </TableCell>
                          <TableCell className="font-mono">
                            {record.open_id}
                          </TableCell>
                          <TableCell className="text-center">
                            <div className="flex items-center justify-center gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => handleEdit(record)}
                              >
                                <PencilIcon className="h-4 w-4" />
                              </Button>
                              <Button
                                variant="destructive"
                                size="sm"
                                onClick={() =>
                                  void handleDelete(record.open_id)
                                }
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

            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">
                共 {filteredOpenIds.length} 条
              </span>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    setCurrentPage((page) => Math.max(1, page - 1))
                  }
                  disabled={currentPage === 1}
                >
                  <ChevronLeftIcon className="h-4 w-4" />
                </Button>
                <span className="text-sm text-foreground">
                  {currentPage} / {totalPages}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    setCurrentPage((page) => Math.min(totalPages, page + 1))
                  }
                  disabled={currentPage === totalPages}
                >
                  <ChevronRightIcon className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>

      <Dialog
        open={batchDialogOpen}
        onOpenChange={(nextOpen) => {
          setBatchDialogOpen(nextOpen);
          if (!nextOpen) {
            resetBatchForm();
          }
        }}
      >
        <DialogContent className="flex max-h-[85vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
          <DialogHeader className="shrink-0 border-b border-border px-6 py-4">
            <DialogTitle>批量新建 OpenID</DialogTitle>
          </DialogHeader>

          <ScrollArea className="min-h-0 flex-1 px-6 py-5">
            <div className="flex flex-col gap-5">
              <FieldGroup>
                <Field>
                  <FieldLabel>FC</FieldLabel>
                  <Select value={batchFcName} onValueChange={setBatchFcName}>
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder="选择 FC" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        {fcs.map((fc) => (
                          <SelectItem key={fc.name} value={fc.name}>
                            {fc.name}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>

                <Field>
                  <FieldLabel>OpenID 列表</FieldLabel>
                  <Textarea
                    rows={12}
                    className="max-h-[42vh] min-h-64 overflow-y-auto"
                    placeholder={"每行一个 OpenID\n例如：\no_xxxxx1\no_xxxxx2\no_xxxxx3"}
                    value={batchOpenIdsText}
                    onChange={(event) => setBatchOpenIdsText(event.target.value)}
                  />
                </Field>
              </FieldGroup>

              <Alert>
                <AlertDescription>
                  会自动去除空行、输入内容中的重复 OpenID，以及数据库中已存在的 OpenID。
                </AlertDescription>
              </Alert>
            </div>
          </ScrollArea>

          <div className="flex shrink-0 items-center justify-end gap-2 border-t border-border px-6 py-4">
            <Button
              variant="outline"
              onClick={() => {
                setBatchDialogOpen(false);
                resetBatchForm();
              }}
              disabled={batchSaving}
            >
              取消
            </Button>
            <Button onClick={() => void handleBatchCreate()} disabled={batchSaving}>
              <PlusIcon className="mr-2 h-4 w-4" />
              {batchSaving ? "保存中..." : "批量新建"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </Dialog>
  );
}
