import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

import type { CommandError } from "@/types";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function getErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    return String((error as CommandError).message);
  }

  return "操作失败，请稍后重试。";
}

export function parseShopcodesInput(value: string): string[] {
  const seen = new Set<string>();
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .filter((line) => {
      if (seen.has(line)) {
        return false;
      }
      seen.add(line);
      return true;
    });
}
