import { useState, useEffect, useMemo } from "react";
import { Trash2Icon, PlusIcon, PencilIcon, ChevronLeftIcon, ChevronRightIcon, SearchIcon } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { getShops, addOrUpdateShop, deleteShop, type ShopRecord } from "@/api/commands";

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
  return SHOP_TYPE_OPTIONS.find(option => option.value === shopType)?.label ?? "Avene";
}

function ShopTypeBadge({ shopType }: { shopType: number }) {
  if (shopType === 2) {
    return (
      <div className="flex items-center justify-center gap-2">
        <Badge className="border-primary/20 bg-primary/12 text-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.35)]">
          Avene
        </Badge>
        <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 shadow-[inset_0_1px_0_rgba(255,255,255,0.45)] dark:border-emerald-400/20 dark:bg-emerald-400/12 dark:text-emerald-300">
          Klorane
        </Badge>
      </div>
    );
  }

  if (shopType === 1) {
    return (
      <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 shadow-[inset_0_1px_0_rgba(255,255,255,0.45)] dark:border-emerald-400/20 dark:bg-emerald-400/12 dark:text-emerald-300">
        Klorane
      </Badge>
    );
  }

  return (
    <Badge className="border-primary/20 bg-primary/12 text-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.35)]">
      Avene
    </Badge>
  );
}

export function ShopManager({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [shops, setShops] = useState<ShopRecord[]>([]);
  const [formData, setFormData] = useState<ShopRecord>(EMPTY_FORM);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchTerm, setSearchTerm] = useState("");
  const [shopTypeFilter, setShopTypeFilter] = useState<string>("all");

  const loadData = async () => {
    try {
      const data = await getShops();
      setShops(data);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    if (open) {
      loadData();
      setCurrentPage(1);
      setSearchTerm("");
      setShopTypeFilter("all");
    }
  }, [open]);

  const filteredShops = useMemo(() => {
    const term = searchTerm.toLowerCase().trim();
    const selectedShopType =
      shopTypeFilter === "all" ? null : Number(shopTypeFilter);

    return shops.filter(shop => 
      (selectedShopType === null || shop.shop_type === selectedShopType) &&
      (!term ||
        shop.province.toLowerCase().includes(term) ||
        shop.city.toLowerCase().includes(term) ||
        shop.shop_code.toLowerCase().includes(term) ||
        shop.fc?.toLowerCase().includes(term) ||
        getShopTypeLabel(shop.shop_type).toLowerCase().includes(term))
    );
  }, [shops, searchTerm, shopTypeFilter]);

  const totalPages = Math.max(1, Math.ceil(filteredShops.length / PAGE_SIZE));
  
  useEffect(() => {
    if (currentPage > totalPages) {
      setCurrentPage(totalPages);
    }
  }, [filteredShops.length, currentPage, totalPages]);

  const handleSave = async () => {
    if (!formData.shop_code.trim()) return;
    try {
      await addOrUpdateShop({
        ...formData,
        fc: formData.fc?.trim() || null,
      });
      setFormData(EMPTY_FORM);
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDelete = async (code: string) => {
    try {
      await deleteShop(code);
      loadData();
    } catch (e) {
      console.error(e);
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
  };

  const currentShops = filteredShops.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-4xl w-[95vw] sm:w-[90vw]">
        <DialogHeader>
          <DialogTitle>门店管理</DialogTitle>
        </DialogHeader>
        
        <div className="space-y-4">
          <div className="grid grid-cols-6 gap-2">
            <Input placeholder="省份" value={formData.province} onChange={(e) => setFormData({ ...formData, province: e.target.value })} />
            <Input placeholder="城市" value={formData.city} onChange={(e) => setFormData({ ...formData, city: e.target.value })} />
            <Input placeholder="门店代码" value={formData.shop_code} onChange={(e) => setFormData({ ...formData, shop_code: e.target.value })} />
            <Input placeholder="FC(选填)" value={formData.fc || ""} onChange={(e) => setFormData({ ...formData, fc: e.target.value })} />
            <Select
              value={String(formData.shop_type)}
              onValueChange={(value) => setFormData({ ...formData, shop_type: Number(value) })}
            >
              <SelectTrigger className="w-full">
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
            <Button onClick={handleSave}><PlusIcon className="w-4 h-4 mr-2" />保存</Button>
          </div>

          <div className="grid grid-cols-1 gap-2 md:grid-cols-[1fr_180px]">
            <div className="relative">
              <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input 
                placeholder="通过关键词搜索门店..." 
                className="pl-9"
                value={searchTerm}
                onChange={(e) => {
                  setSearchTerm(e.target.value);
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
              <SelectTrigger className="w-full">
                <SelectValue placeholder="筛选门店类型" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部类型</SelectItem>
                {SHOP_TYPE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={String(option.value)}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        <ScrollArea className="h-[400px] rounded-md border border-border bg-card">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[150px]">省份</TableHead>
                <TableHead className="w-[150px]">城市</TableHead>
                <TableHead className="w-[200px]">门店代码</TableHead>
                <TableHead className="min-w-[150px]">FC</TableHead>
                <TableHead className="w-[180px]">门店类型</TableHead>
                <TableHead className="w-[120px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {currentShops.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-32 text-center text-muted-foreground">
                    未找到匹配的门店记录
                  </TableCell>
                </TableRow>
              ) : currentShops.map((shop) => (
                <TableRow key={shop.shop_code}>
                  <TableCell>{shop.province}</TableCell>
                  <TableCell>{shop.city}</TableCell>
                  <TableCell className="font-mono">{shop.shop_code}</TableCell>
                  <TableCell>{shop.fc}</TableCell>
                  <TableCell>
                    <ShopTypeBadge shopType={shop.shop_type} />
                  </TableCell>
                  <TableCell>
                    <div className="flex gap-2">
                      <Button variant="outline" size="sm" onClick={() => handleEdit(shop)}>
                        <PencilIcon className="w-4 h-4" />
                      </Button>
                      <Button variant="destructive" size="sm" onClick={() => handleDelete(shop.shop_code)}>
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
            {searchTerm ? `搜索结果: ${filteredShops.length} 条` : `共 ${shops.length} 条记录`}
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
