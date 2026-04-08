import { useMemo, useState, useEffect } from "react";
import { InfoIcon, PlayIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { FormState } from "@/features/useTaskRunner";
import { getFcs, type FcRecord } from "@/api/commands";

interface Props {
  form: FormState;
  updateField: <K extends keyof FormState>(field: K, value: FormState[K]) => void;
  canSubmit: boolean;
  running: boolean;
  runtimeReady: boolean;
  runtimeLoaded: boolean;
  runtimeConfigured: boolean;
  runtimeError: string | null;
  onSubmit: () => void;
}

type FieldName = keyof FormState;

const initialTouched: Record<FieldName, boolean> = {
  sCourseId: false,
  sManagerId: false,
  fc: false,
  shopcodesInput: false,
};

export default function TaskForm({
  form,
  updateField,
  canSubmit,
  running,
  runtimeReady,
  runtimeLoaded,
  runtimeConfigured,
  runtimeError,
  onSubmit,
}: Props) {
  const [touched, setTouched] = useState(initialTouched);
  const [fcs, setFcs] = useState<FcRecord[]>([]);

  useEffect(() => {
    if (!runtimeConfigured) {
      setFcs([]);
      return;
    }

    getFcs().then(setFcs).catch(console.error);
  }, [runtimeConfigured]);

  const errors = useMemo<Record<FieldName, string | null>>(
    () => ({
      sCourseId: form.sCourseId.trim() ? null : "请输入课程 ID。",
      sManagerId: form.sManagerId.trim() ? null : "请选择 FC 经理。",
      fc: form.fc.trim() ? null : "请选择 FC。",
      shopcodesInput: form.shopcodesInput.trim()
        ? null
        : "请至少输入一个 Shop Code，每行一个。",
    }),
    [form]
  );

  const markTouched = (field: FieldName) => {
    setTouched((prev) => ({ ...prev, [field]: true }));
  };

  const showError = (field: FieldName) => touched[field] ? errors[field] : null;

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setTouched({
      sCourseId: true,
      sManagerId: true,
      fc: true,
      shopcodesInput: true,
    });

    if (canSubmit) {
      onSubmit();
    }
  };

  const isBlocked = runtimeLoaded && !runtimeReady;

  return (
    <Card className="shadow-sm transition-all hover:border-primary/30">
      <CardHeader className="gap-2">
        <CardTitle>任务参数</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-5">
        {runtimeError && (
          <Alert variant="destructive">
            <InfoIcon />
            <AlertTitle>暂时无法执行</AlertTitle>
            <AlertDescription>
              运行时状态读取失败，请先确认桌面端后端是否可用。
            </AlertDescription>
          </Alert>
        )}

        {!runtimeError && isBlocked && (
          <Alert className="border-amber-200/80 bg-amber-50/90 text-amber-800 dark:border-amber-900/50 dark:bg-amber-950/25 dark:text-amber-200">
            <InfoIcon />
            <AlertTitle>配置未就绪</AlertTitle>
            <AlertDescription>
              当前配置文件不完整，开始执行按钮会保持禁用。
            </AlertDescription>
          </Alert>
        )}

        <form className="flex flex-col gap-5" onSubmit={handleSubmit}>
          <FieldGroup>
            <div className="grid grid-cols-2 gap-4">
              <Field data-invalid={Boolean(showError("sCourseId"))}>
                <FieldLabel htmlFor="course-id">课程 ID</FieldLabel>
                <Input
                  id="course-id"
                  value={form.sCourseId}
                  disabled={running}
                  aria-invalid={Boolean(showError("sCourseId"))}
                  placeholder="请输入课程 ID"
                  onBlur={() => markTouched("sCourseId")}
                  onChange={(event) => updateField("sCourseId", event.target.value)}
                />
                <FieldError>{showError("sCourseId")}</FieldError>
              </Field>

              <Field data-invalid={Boolean(showError("fc"))}>
                <FieldLabel htmlFor="fc">FC 经理</FieldLabel>
                <Select
                  disabled={running}
                  value={form.fc}
                  onValueChange={(val) => {
                    const fc = fcs.find((f) => f.name === val);
                    if (fc) {
                      updateField("fc", fc.name);
                      updateField("sManagerId", fc.manager_id);
                    }
                    markTouched("fc");
                    markTouched("sManagerId");
                  }}
                >
                  <SelectTrigger id="fc" className="w-full data-[size=default]:h-9">
                    <SelectValue placeholder="请选择 FC 经理" />
                  </SelectTrigger>
                  <SelectContent>
                    {fcs.map((fc) => (
                      <SelectItem key={fc.name} value={fc.name}>
                        {fc.name} (ID: {fc.manager_id})
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <FieldError>{showError("fc") || showError("sManagerId")}</FieldError>
              </Field>
            </div>

            <Field data-invalid={Boolean(showError("shopcodesInput"))}>
              <FieldLabel htmlFor="shopcodes">Shop Code（每行一个）</FieldLabel>
              <Textarea
                id="shopcodes"
                value={form.shopcodesInput}
                disabled={running}
                aria-invalid={Boolean(showError("shopcodesInput"))}
                className="min-h-28"
                placeholder={"10001\n10002\n10003"}
                onBlur={() => markTouched("shopcodesInput")}
                onChange={(event) => updateField("shopcodesInput", event.target.value)}
              />
              <FieldError>{showError("shopcodesInput")}</FieldError>
            </Field>
          </FieldGroup>

          <Button
            type="submit"
            size="lg"
            className="w-full"
            disabled={!canSubmit || running}
          >
            {running ? <Spinner data-icon="inline-start" /> : <PlayIcon data-icon="inline-start" />}
            {running ? "执行中…" : "开始执行"}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
