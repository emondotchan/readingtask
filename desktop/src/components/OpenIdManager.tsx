import { useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import {
  Trash2Icon,
  PlusIcon,
  PencilIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  UploadIcon,
  SearchIcon,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  getOpenIds,
  addOpenId,
  deleteOpenId,
  getFcs,
  importOpenIdsCsv,
  type FcRecord,
} from "@/api/commands";
import type { OpenIdRecord } from "@/types";

const PAGE_SIZE = 50;

export function OpenIdManager({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [openIds, setOpenIds] = useState<OpenIdRecord[]>([]);
  const [fcs, setFcs] = useState<FcRecord[]>([]);
  const [newManagerId, setNewManagerId] = useState("");
  const [newOpenId, setNewOpenId] = useState("");
  const [editingOpenId, setEditingOpenId] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedOpenIds, setSelectedOpenIds] = useState<Set<string>>(new Set());
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const loadData = async () => {
    try {
      const [openIdData, fcData] = await Promise.all([getOpenIds(), getFcs()]);
      setOpenIds(openIdData);
      setFcs(fcData);
      setSelectedOpenIds((previous) => {
        const availableOpenIds = new Set(openIdData.map((record) => record.open_id));
        return new Set([...previous].filter((openId) => availableOpenIds.has(openId)));
      });
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    if (open) {
      loadData();
      setCurrentPage(1);
      setFeedback(null);
      setEditingOpenId(null);
      setSearchTerm("");
      setSelectedOpenIds(new Set());
    }
  }, [open]);

  const fcNameByManagerId = useMemo(
    () => new Map(fcs.map((fc) => [fc.manager_id, fc.name])),
    [fcs],
  );

  const filteredOpenIds = useMemo(() => {
    const term = searchTerm.toLowerCase().trim();
    if (!term) {
      return openIds;
    }

    return openIds.filter((record) => {
      const fcName = fcNameByManagerId.get(record.manager_id) ?? "未匹配";
      return (
        record.manager_id.toLowerCase().includes(term) ||
        record.open_id.toLowerCase().includes(term) ||
        fcName.toLowerCase().includes(term)
      );
    });
  }, [fcNameByManagerId, openIds, searchTerm]);

  const totalPages = Math.max(1, Math.ceil(filteredOpenIds.length / PAGE_SIZE));
  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [filteredOpenIds.length, currentPage, totalPages]);

  const resetForm = () => {
    setNewManagerId("");
    setNewOpenId("");
    setEditingOpenId(null);
  };

  const handleSave = async () => {
    if (!newManagerId.trim() || !newOpenId.trim()) return;

    const trimmedManagerId = newManagerId.trim();
    const trimmedOpenId = newOpenId.trim();

    try {
      if (editingOpenId && editingOpenId !== trimmedOpenId) {
        await deleteOpenId(editingOpenId);
      }

      await addOpenId({
        manager_id: trimmedManagerId,
        open_id: trimmedOpenId,
      });

      resetForm();
      setFeedback(editingOpenId ? "OpenID 已更新" : "OpenID 已保存");
      await loadData();
    } catch (e) {
      console.error(e);
      setFeedback(e instanceof Error ? e.message : String(e));
    }
  };

  const handleEdit = (record: OpenIdRecord) => {
    setNewManagerId(record.manager_id);
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
      setFeedback("OpenID 已删除");
      await loadData();
    } catch (e) {
      console.error(e);
      setFeedback(e instanceof Error ? e.message : String(e));
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
    } catch (e) {
      console.error(e);
      setFeedback(e instanceof Error ? e.message : String(e));
    }
  };

  const handleImportFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) {
      return;
    }

    try {
      const count = await importOpenIdsCsv(await file.text());
      setFeedback(`成功导入 ${count} 条 OpenID 记录`);
      await loadData();
    } catch (e) {
      console.error(e);
      setFeedback(e instanceof Error ? e.message : String(e));
    } finally {
      event.target.value = "";
    }
  };

  const currentIds = filteredOpenIds.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);
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
      <DialogContent className="sm:max-w-4xl w-[95vw] sm:w-[92vw]">
        <DialogHeader>
          <DialogTitle>OpenID 管理</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          {feedback && (
            <Alert>
              <AlertTitle>操作结果</AlertTitle>
              <AlertDescription>{feedback}</AlertDescription>
            </Alert>
          )}
          <div className="grid grid-cols-1 md:grid-cols-[160px_1fr_auto_auto_auto] gap-2">
            <Input
              placeholder="ManagerID"
              value={newManagerId}
              onChange={(e) => setNewManagerId(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSave()}
            />
            <Input
              placeholder="输入新的 OpenID"
              value={newOpenId}
              onChange={(e) => setNewOpenId(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSave()}
            />
            <Button onClick={handleSave}>
              {editingOpenId ? (
                <PencilIcon className="w-4 h-4 mr-2" />
              ) : (
                <PlusIcon className="w-4 h-4 mr-2" />
              )}
              {editingOpenId ? "保存编辑" : "新增"}
            </Button>
            <Button variant="outline" onClick={resetForm} disabled={!newManagerId && !newOpenId && !editingOpenId}>
              取消
            </Button>
            <Button variant="outline" onClick={() => fileInputRef.current?.click()}>
              <UploadIcon className="w-4 h-4 mr-2" />
              导入 CSV
            </Button>
            <input
              ref={fileInputRef}
              type="file"
              accept=".csv,text/csv"
              className="hidden"
              onChange={handleImportFile}
            />
          </div>
          <p className="text-xs text-muted-foreground">
            CSV 第一列为 ManagerID，第二列为 OpenID。FC 列根据 ManagerID 在 FC 配置中自动匹配。
          </p>
          <div className="relative">
            <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="搜索 ManagerID / FC / OpenID"
              className="pl-9"
              value={searchTerm}
              onChange={(e) => {
                setSearchTerm(e.target.value);
                setCurrentPage(1);
                setSelectedOpenIds(new Set());
              }}
            />
          </div>
          <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border/70 bg-muted/20 px-3 py-2">
            <span className="text-xs text-muted-foreground">
              已选 {selectedCount} 条，当前页 {currentIds.length} 条
            </span>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => toggleCurrentPageSelection(!allCurrentPageSelected)}
                disabled={currentIds.length === 0}
              >
                {allCurrentPageSelected ? "取消全选当前页" : "全选当前页"}
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onClick={handleDeleteSelected}
                disabled={selectedCount === 0}
              >
                <Trash2Icon className="w-4 h-4 mr-2" />
                删除选中
              </Button>
            </div>
          </div>
        </div>
        <ScrollArea className="h-[400px] rounded-md border border-border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-12 text-center">
                  <input
                    type="checkbox"
                    className="size-4 accent-primary"
                    aria-label="全选当前页 OpenID"
                    checked={allCurrentPageSelected}
                    onChange={(event) => toggleCurrentPageSelection(event.target.checked)}
                  />
                </TableHead>
                <TableHead className="w-[140px]">ManagerID</TableHead>
                <TableHead className="w-[160px]">FC</TableHead>
                <TableHead className="text-left">OpenID</TableHead>
                <TableHead className="w-[160px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {currentIds.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-muted-foreground">
                    暂无 OpenID 记录
                  </TableCell>
                </TableRow>
              ) : currentIds.map((record) => (
                <TableRow key={record.open_id}>
                  <TableCell className="text-center">
                    <input
                      type="checkbox"
                      className="size-4 accent-primary"
                      aria-label={`选择 OpenID ${record.open_id}`}
                      checked={selectedOpenIds.has(record.open_id)}
                      onChange={(event) =>
                        toggleOpenIdSelection(record.open_id, event.target.checked)
                      }
                    />
                  </TableCell>
                  <TableCell className="font-mono">{record.manager_id || "-"}</TableCell>
                  <TableCell>{fcNameByManagerId.get(record.manager_id) ?? "未匹配"}</TableCell>
                  <TableCell className="text-left font-mono">{record.open_id}</TableCell>
                  <TableCell>
                    <div className="flex items-center justify-center gap-2">
                      <Button variant="outline" size="sm" onClick={() => handleEdit(record)}>
                        <PencilIcon className="w-4 h-4" />
                      </Button>
                      <Button variant="destructive" size="sm" onClick={() => handleDelete(record.open_id)}>
                        <Trash2Icon className="w-4 h-4" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </ScrollArea>
        <div className="flex items-center justify-between mt-2">
          <span className="text-sm text-muted-foreground">
            {searchTerm ? `搜索结果: ${filteredOpenIds.length} 条` : `共 ${openIds.length} 条记录`}
          </span>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
              disabled={currentPage === 1}
            >
              <ChevronLeftIcon className="w-4 h-4" />
            </Button>
            <span className="text-sm text-foreground">
              {currentPage} / {totalPages}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
              disabled={currentPage === totalPages}
            >
              <ChevronRightIcon className="w-4 h-4" />
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
