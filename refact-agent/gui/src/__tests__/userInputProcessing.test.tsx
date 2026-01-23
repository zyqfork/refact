import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as path from "path";

describe("UserInput processing", () => {
  it("uses iterative processLines (no recursion)", () => {
    const filePath = path.resolve(
      __dirname,
      "../components/ChatContent/UserInput.tsx",
    );
    const content = fs.readFileSync(filePath, "utf-8");

    expect(content).toContain("while (i < lines.length)");
    expect(content).not.toMatch(
      /function processLines\([^)]*\):[^{]*\{[^}]*return processLines\(/,
    );
  });

  it("uses iterative processUserInputArray (no recursion)", () => {
    const filePath = path.resolve(
      __dirname,
      "../components/ChatContent/UserInput.tsx",
    );
    const content = fs.readFileSync(filePath, "utf-8");

    expect(content).toContain("while (i < items.length)");
    expect(content).not.toMatch(/return processUserInputArray\(.*memo\.concat/);
  });

  it("uses push instead of concat for building arrays", () => {
    const filePath = path.resolve(
      __dirname,
      "../components/ChatContent/UserInput.tsx",
    );
    const content = fs.readFileSync(filePath, "utf-8");

    expect(content).toContain("result.push(");
    expect(content).not.toMatch(/processedLinesMemo\.concat/);
  });
});

describe("URL sanitization in AssistantInput", () => {
  it("filters citations by URL protocol", () => {
    const filePath = path.resolve(
      __dirname,
      "../components/ChatContent/AssistantInput.tsx",
    );
    const content = fs.readFileSync(filePath, "utf-8");

    expect(content).toContain('url.protocol === "http:"');
    expect(content).toContain('url.protocol === "https:"');
  });
});

describe("DiffTitle uses numeric counts", () => {
  it("displays counts instead of repeated characters", () => {
    const filePath = path.resolve(
      __dirname,
      "../components/ChatContent/DiffContent.tsx",
    );
    const content = fs.readFileSync(filePath, "utf-8");

    expect(content).toContain("addCount");
    expect(content).toContain("removeCount");
    const greenIdx = content.indexOf("+{addCount}");
    const redIdx = content.indexOf("-{removeCount}");
    expect(greenIdx).toBeLessThan(redIdx);
    expect(content).not.toContain('"+".repeat');
    expect(content).not.toContain('"-".repeat');
  });
});
