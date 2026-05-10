import { useState, useEffect, useMemo, type ChangeEvent } from "react";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  SearchIcon,
  StoreIcon,
  Trash2Icon,
  UploadIcon,
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
import {
  getShops,
  deleteAllShops,
  importShops,
  updateShopTypes,
  type ShopRecord,
} from "@/api/commands";
import { getErrorMessage } from "@/lib/utils";

const PAGE_SIZE = 50;
const SHOP_TYPE_OPTIONS = [
  { value: 0, label: "Avene" },
  { value: 1, label: "Klorane" },
  { value: 2, label: "Avene + Klorane" },
];

type ImportedShop = {
  Province?: unknown;
  City?: unknown;
  ShopCode?: unknown;
  ShopName?: unknown;
  FC?: unknown;
  ShopType?: unknown;
};

function normalizeImportedShops(payload: unknown): ShopRecord[] {
  if (!Array.isArray(payload)) {
    throw new Error("文件内容必须是 JSON 数组");
  }

  const shops = new Map<string, ShopRecord>();

  payload.forEach((item, index) => {
    if (!item || typeof item !== "object") {
      throw new Error(`第 ${index + 1} 条门店不是有效对象`);
    }

    const source = item as ImportedShop;
    const shopCode = String(source.ShopCode ?? "").trim();

    if (!shopCode) {
      throw new Error(`第 ${index + 1} 条门店缺少 ShopCode`);
    }

    const shopType = Number(source.ShopType ?? 0);

    shops.set(shopCode, {
      province: String(source.Province ?? "").trim(),
      city: String(source.City ?? "").trim(),
      shop_code: shopCode,
      shop_name: String(source.ShopName ?? "").trim(),
      fc: String(source.FC ?? "").trim() || null,
      shop_type: Number.isFinite(shopType) ? shopType : 0,
    });
  });

  return Array.from(shops.values());
}

function parseShopCodeLines(text: string): string[] {
  return Array.from(
    new Set(
      text
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter(Boolean),
    ),
  );
}

function getShopTypeLabel(shopType: number) {
  return (
    SHOP_TYPE_OPTIONS.find((option) => option.value === shopType)?.label ??
    "Avene"
  );
}

function ShopTypeBadge({ shopType }: { shopType: number }) {
  if (shopType === 2) {
    return (
      <div className="flex items-center gap-2">
        <Badge className="border-primary/20 bg-primary/12 text-primary">
          Avene
        </Badge>
        <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-400/20 dark:bg-emerald-400/12 dark:text-emerald-300">
          Klorane
        </Badge>
      </div>
    );
  }

  if (shopType === 1) {
    return (
      <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-400/20 dark:bg-emerald-400/12 dark:text-emerald-300">
        Klorane
      </Badge>
    );
  }

  return (
    <Badge className="border-primary/20 bg-primary/12 text-primary">
      Avene
    </Badge>
  );
}

export function ShopManager({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [shops, setShops] = useState<ShopRecord[]>([]);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchTerm, setSearchTerm] = useState("");
  const [shopTypeFilter, setShopTypeFilter] = useState<string>("all");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [isUpdatingKlorane, setIsUpdatingKlorane] = useState(false);
  const [isDeletingAll, setIsDeletingAll] = useState(false);

  const loadData = async () => {
    try {
      setShops(await getShops());
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
    setShopTypeFilter("all");
    setFeedback(null);
    setErrorMessage(null);
  }, [open]);

  const filteredShops = useMemo(() => {
    const term = searchTerm.toLowerCase().trim();
    const selectedShopType =
      shopTypeFilter === "all" ? null : Number(shopTypeFilter);

    return shops.filter(
      (shop) =>
        (selectedShopType === null || shop.shop_type === selectedShopType) &&
        (!term ||
          shop.province.toLowerCase().includes(term) ||
          shop.city.toLowerCase().includes(term) ||
          shop.shop_code.toLowerCase().includes(term) ||
          shop.shop_name.toLowerCase().includes(term) ||
          shop.fc?.toLowerCase().includes(term) ||
          getShopTypeLabel(shop.shop_type).toLowerCase().includes(term)),
    );
  }, [shops, searchTerm, shopTypeFilter]);

  const totalPages = Math.max(1, Math.ceil(filteredShops.length / PAGE_SIZE));

  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [currentPage, totalPages]);

  const handleDeleteAll = async () => {
    if (shops.length === 0) {
      return;
    }

    const confirmed = window.confirm(
      `确认删除全部 ${shops.length} 条门店记录？此操作不会删除历史执行结果。`,
    );

    if (!confirmed) {
      return;
    }

    try {
      setIsDeletingAll(true);
      await deleteAllShops();
      setFeedback("已删除全部门店");
      setErrorMessage(null);
      setCurrentPage(1);
      await loadData();
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsDeletingAll(false);
    }
  };

  const handleImportFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";

    if (!file) {
      return;
    }

    try {
      setIsImporting(true);
      const text = await file.text();
      const importedShops = normalizeImportedShops(JSON.parse(text));
      const importedCount = await importShops(importedShops);
      setFeedback(`已导入 ${importedCount} 条门店`);
      setErrorMessage(null);
      setCurrentPage(1);
      await loadData();
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsImporting(false);
    }
  };

  const handleKloraneFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";

    if (!file) {
      return;
    }

    try {
      setIsUpdatingKlorane(true);
      const shopCodes = parseShopCodeLines(await file.text());

      if (shopCodes.length === 0) {
        throw new Error("Klorane 文件不能为空");
      }

      const updatedCount = await updateShopTypes(shopCodes, 2);
      setFeedback(
        `已将 ${updatedCount} 条门店更新为 Avene + Klorane，共读取 ${shopCodes.length} 个 ShopCode`,
      );
      setErrorMessage(null);
      await loadData();
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsUpdatingKlorane(false);
    }
  };

  const currentShops = filteredShops.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE,
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-5xl w-[95vw] max-h-[90vh] overflow-hidden gap-0 flex flex-col p-0">
        <DialogHeader className="px-6 py-4 border-b border-border shrink-0">
          <DialogTitle className="flex items-center gap-2 text-xl">
            <StoreIcon className="size-5 text-muted-foreground" />
            门店管理
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto p-6 space-y-6 bg-muted/10">
          {(feedback || errorMessage) && (
            <Alert variant={errorMessage ? "destructive" : "default"}>
              <AlertDescription>{errorMessage ?? feedback}</AlertDescription>
            </Alert>
          )}

          <div className="space-y-4">
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex items-center gap-2">
                <h3 className="text-sm font-semibold text-foreground">
                  门店列表
                </h3>
                <Badge variant="secondary" className="font-mono">
                  {filteredShops.length}
                </Badge>
              </div>
              <div className="flex w-full flex-col gap-3 sm:w-auto sm:flex-row sm:items-center">
                <Button asChild variant="outline" disabled={isImporting}>
                  <label>
                    <UploadIcon data-icon="inline-start" />
                    {isImporting ? "导入中" : "导入 txt"}
                    <input
                      type="file"
                      accept=".txt,.json,application/json,text/plain"
                      className="sr-only"
                      onChange={handleImportFile}
                    />
                  </label>
                </Button>
                <Button
                  asChild
                  variant="outline"
                  disabled={isUpdatingKlorane}
                >
                  <label>
                    <UploadIcon data-icon="inline-start" />
                    {isUpdatingKlorane ? "更新中" : "上传 Klorane"}
                    <input
                      type="file"
                      accept=".txt,text/plain"
                      className="sr-only"
                      disabled={isUpdatingKlorane}
                      onChange={handleKloraneFile}
                    />
                  </label>
                </Button>
                <Button
                  variant="destructive"
                  onClick={handleDeleteAll}
                  disabled={shops.length === 0 || isDeletingAll}
                >
                  <Trash2Icon data-icon="inline-start" />
                  {isDeletingAll ? "删除中" : "删除全部"}
                </Button>
                <div className="relative w-full sm:w-[260px]">
                  <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
                  <Input
                    placeholder="搜索门店"
                    className="pl-9 bg-card"
                    value={searchTerm}
                    onChange={(event) => {
                      setSearchTerm(event.target.value);
                      setCurrentPage(1);
                    }}
                  />
                </div>
                <Select
                  value={shopTypeFilter}
                  onValueChange={(value) => {
                    setShopTypeFilter(value);
                    setCurrentPage(1);
                  }}
                >
                  <SelectTrigger className="w-full sm:w-[180px] bg-card">
                    <SelectValue placeholder="筛选门店类型" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部类型</SelectItem>
                    {SHOP_TYPE_OPTIONS.map((option) => (
                      <SelectItem
                        key={option.value}
                        value={String(option.value)}
                      >
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
              <ScrollArea className="h-[400px]">
                <Table>
                  <TableHeader>
                    <TableRow className="bg-muted/50 hover:bg-muted/50">
                      <TableHead className="w-[140px] sticky top-0 bg-muted/90">
                        省份
                      </TableHead>
                      <TableHead className="w-[140px] sticky top-0 bg-muted/90">
                        城市
                      </TableHead>
                      <TableHead className="w-[200px] sticky top-0 bg-muted/90">
                        门店代码
                      </TableHead>
                      <TableHead className="sticky top-0 bg-muted/90">
                        门店名称
                      </TableHead>
                      <TableHead className="sticky top-0 bg-muted/90">
                        FC
                      </TableHead>
                      <TableHead className="w-[180px] sticky top-0 bg-muted/90">
                        门店类型
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {currentShops.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={6}
                          className="h-32 text-center text-muted-foreground"
                        >
                          暂无记录
                        </TableCell>
                      </TableRow>
                    ) : (
                      currentShops.map((shop) => (
                        <TableRow
                          key={shop.shop_code}
                          className="group hover:bg-muted/40"
                        >
                          <TableCell>{shop.province}</TableCell>
                          <TableCell>{shop.city}</TableCell>
                          <TableCell className="font-mono">
                            {shop.shop_code}
                          </TableCell>
                          <TableCell>{shop.shop_name}</TableCell>
                          <TableCell>{shop.fc}</TableCell>
                          <TableCell>
                            <ShopTypeBadge shopType={shop.shop_type} />
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
                共 {filteredShops.length} 条
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
                  <ChevronLeftIcon />
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
                  <ChevronRightIcon />
                </Button>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
