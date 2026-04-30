import { describe, it, expect } from "vitest";
import { cn, getErrorMessage } from "./utils";

describe("utils", () => {
  describe("cn", () => {
    it("should merge tailwind classes properly", () => {
      expect(cn("px-2 py-1", "bg-blue-500")).toBe("px-2 py-1 bg-blue-500");
    });

    it("should handle conditional classes", () => {
      expect(cn("px-2", true && "py-1", false && "bg-red-500")).toBe("px-2 py-1");
    });

    it("should resolve tailwind class conflicts using tailwind-merge", () => {
      expect(cn("px-2 py-1", "px-4")).toBe("py-1 px-4");
    });
  });

  describe("getErrorMessage", () => {
    it("should extract message from Error object", () => {
      const error = new Error("Test error message");
      expect(getErrorMessage(error)).toBe("Test error message");
    });

    it("should convert string to string", () => {
      expect(getErrorMessage("Just a string error")).toBe("Just a string error");
    });

    it("should handle null or undefined gracefully", () => {
      expect(getErrorMessage(null)).toBe("null");
      expect(getErrorMessage(undefined)).toBe("undefined");
    });
  });
});
