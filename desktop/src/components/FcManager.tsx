import { useState, useEffect, useMemo, type FormEvent } from "react";
import { Trash2Icon, PlusIcon, PencilIcon, ChevronLeftIcon, ChevronRightIcon, SearchIcon } from "lucide-react";
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
import { getFcs, addOrUpdateFc, deleteFc, type FcRecord } from "@/api/commands";

const PAGE_SIZE = 50;

export function FcManager({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [fcs, setFcs] = useState<FcRecord[]>([]);
  const [formData, setFormData] = useState<FcRecord>({ name: "", manager_id: "" });
  const [editingName, setEditingName] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchTerm, setSearchTerm] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);

  const resetForm = () => {
    setFormData({ name: "", manager_id: "" });
    setEditingName(null);
  };

  const loadData = async () => {
    try {
      const data = await getFcs();
      setFcs(data);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    if (open) {
      loadData();
      setCurrentPage(1);
      setSearchTerm("");
      setFeedback(null);
      resetForm();
    }
  }, [open]);

  const filteredFcs = useMemo(() => {
    const term = searchTerm.toLowerCase().trim();
    if (!term) return fcs;
    return fcs.filter(fc => 
      fc.name.toLowerCase().includes(term) ||
      fc.manager_id.toLowerCase().includes(term)
    );
  }, [fcs, searchTerm]);

  const totalPages = Math.max(1, Math.ceil(filteredFcs.length / PAGE_SIZE));
  
  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [filteredFcs.length, currentPage, totalPages]);

  const handleSave = async (event?: FormEvent) => {
    event?.preventDefault();

    if (!formData.name.trim() || !formData.manager_id.trim()) {
      setFeedback("请先填写 FC 名称和 Manager ID。");
      return;
    }

    try {
      await addOrUpdateFc({
        fc: {
          name: formData.name.trim(),
          manager_id: formData.manager_id.trim(),
        },
        previous_name: editingName,
      });
      resetForm();
      setFeedback(editingName ? "FC 经理已更新" : "FC 经理已保存");
      await loadData();
    } catch (e) {
      console.error(e);
      setFeedback(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async (name: string) => {
    try {
      await deleteFc(name);
      if (editingName === name) {
        resetForm();
      }
      await loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleEdit = (fc: FcRecord) => {
    setFormData({
      name: fc.name,
      manager_id: fc.manager_id,
    });
    setEditingName(fc.name);
  };

  const currentFcs = filteredFcs.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl w-[95vw] sm:w-[90vw]">
        <DialogHeader>
          <DialogTitle>FC 经理管理</DialogTitle>
        </DialogHeader>
        
        <div className="space-y-4">
          {feedback && (
            <Alert>
              <AlertTitle>提示</AlertTitle>
              <AlertDescription>{feedback}</AlertDescription>
            </Alert>
          )}

          <form className="grid grid-cols-3 gap-2" onSubmit={handleSave}>
            <Input
              placeholder="姓名 (如: 周凡琪)"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
            />
            <Input
              placeholder="Manager ID"
              value={formData.manager_id}
              onChange={(e) => setFormData({ ...formData, manager_id: e.target.value })}
            />
            <div className="flex gap-2">
              <Button type="submit" className="flex-1">
                {editingName ? <PencilIcon className="w-4 h-4 mr-2" /> : <PlusIcon className="w-4 h-4 mr-2" />}
                {editingName ? "更新" : "保存"}
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={resetForm}
                disabled={!formData.name && !formData.manager_id && !editingName}
              >
                取消
              </Button>
            </div>
          </form>

          <div className="relative">
            <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input 
              placeholder="搜索经理姓名或 ID..." 
              className="pl-9"
              value={searchTerm}
              onChange={(e) => {
                setSearchTerm(e.target.value);
                setCurrentPage(1);
              }}
            />
          </div>
        </div>

        <ScrollArea className="h-[400px] rounded-md border border-border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[180px]">姓名</TableHead>
                <TableHead className="w-[180px]">Manager ID</TableHead>
                <TableHead className="w-[120px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {currentFcs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} className="h-32 text-center text-muted-foreground">
                    未找到匹配的经理记录
                  </TableCell>
                </TableRow>
              ) : currentFcs.map((fc) => (
                <TableRow key={fc.name}>
                  <TableCell className="font-medium">{fc.name}</TableCell>
                  <TableCell className="font-mono">{fc.manager_id}</TableCell>
                  <TableCell>
                    <div className="flex gap-2">
                      <Button variant="outline" size="sm" onClick={() => handleEdit(fc)}>
                        <PencilIcon className="w-4 h-4" />
                      </Button>
                      <Button variant="destructive" size="sm" onClick={() => handleDelete(fc.name)}>
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
            {searchTerm ? `搜索结果: ${filteredFcs.length} 条` : `共 ${fcs.length} 条记录`}
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
