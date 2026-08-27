import { MoonIcon, SunIcon, MonitorIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTheme } from "@/components/theme-provider";

export function ModeToggle() {
  const { theme, setTheme } = useTheme();
  const activeClassName = "bg-muted text-primary";
  const inactiveClassName = "text-muted-foreground";

  return (
    <div className="flex items-center rounded-lg border border-border bg-card p-1 shadow-sm">
      <Button
        variant="ghost"
        size="sm"
        className={`h-8 w-8 px-0 ${theme === "light" ? activeClassName : inactiveClassName}`}
        onClick={() => setTheme("light")}
      >
        <SunIcon className="h-4 w-4" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className={`h-8 w-8 px-0 ${theme === "dark" ? activeClassName : inactiveClassName}`}
        onClick={() => setTheme("dark")}
      >
        <MoonIcon className="h-4 w-4" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className={`h-8 w-8 px-0 ${theme === "system" ? activeClassName : inactiveClassName}`}
        onClick={() => setTheme("system")}
      >
        <MonitorIcon className="h-4 w-4" />
      </Button>
    </div>
  );
}
