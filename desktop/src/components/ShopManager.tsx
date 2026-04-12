import { useState, useEffect, useMemo } from "react";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  PencilIcon,
  PlusIcon,
  SearchIcon,
  StoreIcon,
  Trash2Icon,
} from "lucide-react";

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
  addOrUpdateShop,
  deleteShop,
  type ShopRecord,
} from "@/api/commands";

const PAGE_SIZE = 50;
const SHOP_TYPE_OPTIONS = [
  { value: 0, label: "Avene" },
  { value: 1, label: "Klorane" },
  { value: 2, label: "Avene + Klorane" },
];

const EMPTY_FORM: ShopRecord = {
  province: "",
  city: "",
  shop_code: "",
  fc: "",
  shop_type: 0,
};

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
  const [formData, setFormData] = useState<ShopRecord>(EMPTY_FORM);
  const [editingCode, setEditingCode] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchTerm, setSearchTerm] = useState("");
  const [shopTypeFilter, setShopTypeFilter] = useState<string>("all");

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
    setFormData(EMPTY_FORM);
    setEditingCode(null);
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

  const handleSave = async () => {
    if (!formData.shop_code.trim()) return;

    try {
      await addOrUpdateShop({
        ...formData,
        fc: formData.fc?.trim() || null,
      });
      setFormData(EMPTY_FORM);
      setEditingCode(null);
      await loadData();
    } catch (error) {
      console.error(error);
    }
  };

  const handleDelete = async (code: string) => {
    try {
      await deleteShop(code);
      if (editingCode === code) {
        setFormData(EMPTY_FORM);
        setEditingCode(null);
      }
      await loadData();
    } catch (error) {
      console.error(error);
    }
  };

  const handleEdit = (shop: ShopRecord) => {
    setFormData({
      province: shop.province,
      city: shop.city,
      shop_code: shop.shop_code,
      fc: shop.fc || "",
      shop_type: shop.shop_type ?? 0,
    });
    setEditingCode(shop.shop_code);
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
          <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-[1fr_1fr_1.2fr_1fr_1fr_auto]">
              <Input
                placeholder="省份"
                value={formData.province}
                onChange={(event) =>
                  setFormData({ ...formData, province: event.target.value })
                }
              />
              <Input
                placeholder="城市"
                value={formData.city}
                onChange={(event) =>
                  setFormData({ ...formData, city: event.target.value })
                }
              />
              <Input
                placeholder="门店代码"
                value={formData.shop_code}
                onChange={(event) =>
                  setFormData({ ...formData, shop_code: event.target.value })
                }
              />
              <Input
                placeholder="FC"
                value={formData.fc || ""}
                onChange={(event) =>
                  setFormData({ ...formData, fc: event.target.value })
                }
              />
              <Select
                value={String(formData.shop_type)}
                onValueChange={(value) =>
                  setFormData({ ...formData, shop_type: Number(value) })
                }
              >
                <SelectTrigger>
                  <SelectValue placeholder="门店类型" />
                </SelectTrigger>
                <SelectContent>
                  {SHOP_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={String(option.value)}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <div className="flex gap-2">
                <Button onClick={handleSave} className="w-[100px]">
                  {editingCode ? (
                    <PencilIcon className="mr-2 h-4 w-4" />
                  ) : (
                    <PlusIcon className="mr-2 h-4 w-4" />
                  )}
                  {editingCode ? "更新" : "添加"}
                </Button>
                {editingCode && (
                  <Button
                    variant="outline"
                    onClick={() => {
                      setFormData(EMPTY_FORM);
                      setEditingCode(null);
                    }}
                  >
                    取消
                  </Button>
                )}
              </div>
            </div>
          </div>

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
                        FC
                      </TableHead>
                      <TableHead className="w-[180px] sticky top-0 bg-muted/90">
                        门店类型
                      </TableHead>
                      <TableHead className="w-[120px] sticky top-0 bg-muted/90 text-center">
                        操作
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
                          <TableCell>{shop.fc}</TableCell>
                          <TableCell>
                            <ShopTypeBadge shopType={shop.shop_type} />
                          </TableCell>
                          <TableCell className="text-center">
                            <div className="flex items-center justify-center gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => handleEdit(shop)}
                              >
                                <PencilIcon className="h-4 w-4" />
                              </Button>
                              <Button
                                variant="destructive"
                                size="sm"
                                onClick={() => handleDelete(shop.shop_code)}
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
    </Dialog>
  );
}
