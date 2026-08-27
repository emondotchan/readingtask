import { useState, useEffect, useMemo, type FormEvent } from "react";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  PencilIcon,
  PlusIcon,
  SearchIcon,
  Trash2Icon,
  UsersIcon,
} from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
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
import { getFcs, addOrUpdateFc, deleteFc, type FcRecord } from "@/api/commands";
import { getErrorMessage } from "@/lib/utils";

const PAGE_SIZE = 50;

export function FcManager({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [fcs, setFcs] = useState<FcRecord[]>([]);
  const [formData, setFormData] = useState<FcRecord>({
    name: "",
  });
  const [editingName, setEditingName] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchTerm, setSearchTerm] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);

  const resetForm = () => {
    setFormData({ name: "" });
    setEditingName(null);
  };

  const loadData = async () => {
    try {
      setFcs(await getFcs());
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
    setSearchTerm("");
    setFeedback(null);
    resetForm();
  }, [open]);

  const filteredFcs = useMemo(() => {
    const term = searchTerm.toLowerCase().trim();
    if (!term) return fcs;
    return fcs.filter((fc) => fc.name.toLowerCase().includes(term));
  }, [fcs, searchTerm]);

  const totalPages = Math.max(1, Math.ceil(filteredFcs.length / PAGE_SIZE));

  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [currentPage, totalPages]);

  const handleSave = async (event?: FormEvent) => {
    event?.preventDefault();

    if (!formData.name.trim()) {
      return;
    }

    try {
      await addOrUpdateFc({
        fc: {
          name: formData.name.trim(),
        },
        previous_name: editingName,
      });
      resetForm();
      setFeedback(editingName ? "FC 已更新" : "FC 已保存");
      await loadData();
    } catch (error) {
      console.error(error);
      setFeedback(getErrorMessage(error));
    }
  };

  const handleDelete = async (name: string) => {
    try {
      await deleteFc(name);
      if (editingName === name) {
        resetForm();
      }
      await loadData();
    } catch (error) {
      console.error(error);
    }
  };

  const handleEdit = (fc: FcRecord) => {
    setFormData(fc);
    setEditingName(fc.name);
    setFeedback(null);
  };

  const currentFcs = filteredFcs.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-4xl w-[95vw] max-h-[90vh] overflow-hidden gap-0 flex flex-col p-0">
        <DialogHeader className="px-6 py-4 border-b border-border shrink-0">
          <DialogTitle className="flex items-center gap-2 text-xl">
            <UsersIcon className="size-5 text-muted-foreground" />
            FC 管理
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto p-6 space-y-6 bg-muted/10">
          {feedback && (
            <Alert className="bg-background">
              <AlertDescription>{feedback}</AlertDescription>
            </Alert>
          )}

          <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
            <form className="grid grid-cols-1 gap-3 md:grid-cols-[1fr_auto]" onSubmit={handleSave}>
              <Input
                placeholder="姓名"
                value={formData.name}
                onChange={(event) => setFormData({ ...formData, name: event.target.value })}
              />
              <div className="flex gap-2">
                <Button type="submit" className="w-[100px]">
                  {editingName ? (
                    <PencilIcon className="mr-2 h-4 w-4" />
                  ) : (
                    <PlusIcon className="mr-2 h-4 w-4" />
                  )}
                  {editingName ? "更新" : "添加"}
                </Button>
                {editingName && (
                  <Button type="button" variant="outline" onClick={resetForm}>
                    取消
                  </Button>
                )}
              </div>
            </form>
          </div>

          <div className="space-y-4">
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex items-center gap-2">
                <h3 className="text-sm font-semibold text-foreground">FC 列表</h3>
                <Badge variant="secondary" className="font-mono">
                  {filteredFcs.length}
                </Badge>
              </div>
              <div className="relative w-full sm:w-[260px]">
                <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder="搜索姓名"
                  className="pl-9 bg-card"
                  value={searchTerm}
                  onChange={(event) => {
                    setSearchTerm(event.target.value);
                    setCurrentPage(1);
                  }}
                />
              </div>
            </div>

            <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
              <ScrollArea className="h-[400px]">
                <Table>
                  <TableHeader>
                    <TableRow className="bg-muted/50 hover:bg-muted/50">
                      <TableHead className="w-[180px] sticky top-0 bg-muted/90">姓名</TableHead>
                      <TableHead className="w-[120px] sticky top-0 bg-muted/90 text-center">
                        操作
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {currentFcs.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={2} className="h-32 text-center text-muted-foreground">
                          暂无记录
                        </TableCell>
                      </TableRow>
                    ) : (
                      currentFcs.map((fc) => (
                        <TableRow key={fc.name} className="group hover:bg-muted/40">
                          <TableCell className="font-medium">{fc.name}</TableCell>
                          <TableCell className="text-center">
                            <div className="flex items-center justify-center gap-2">
                              <Button variant="outline" size="sm" onClick={() => handleEdit(fc)}>
                                <PencilIcon className="h-4 w-4" />
                              </Button>
                              <Button
                                variant="destructive"
                                size="sm"
                                onClick={() => handleDelete(fc.name)}
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
              <span className="text-sm text-muted-foreground">共 {filteredFcs.length} 条</span>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setCurrentPage((page) => Math.max(1, page - 1))}
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
                  onClick={() => setCurrentPage((page) => Math.min(totalPages, page + 1))}
                  disabled={currentPage === totalPages}
                >
                  <ChevronRightIcon className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
