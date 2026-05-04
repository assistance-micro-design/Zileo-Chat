/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setTheme: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("$lib/i18n", () => ({
  setLanguageTag: vi.fn(),
  isAvailableLanguageTag: (tag: string) => tag === "en" || tag === "fr",
}));

import { theme } from "../theme";
import { localeStore, locale } from "../locale";

function mockLocalStorageFailure(): void {
  vi.spyOn(window.localStorage.__proto__, "getItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
  vi.spyOn(window.localStorage.__proto__, "setItem").mockImplementation(() => {
    throw new Error("storage unavailable");
  });
}

describe("runtime preference stores", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("lang");
    theme.cleanup();
  });

  it("theme.setTheme remains usable when localStorage throws", () => {
    mockLocalStorageFailure();

    expect(() => theme.setTheme("dark")).not.toThrow();
    expect(get(theme)).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("theme.init works without matchMedia support", () => {
    const original = window.matchMedia;
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: undefined,
    });

    expect(() => theme.init()).not.toThrow();
    expect(get(theme)).toBe("light");

    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: original,
    });
  });

  it("localeStore.setLocale remains usable when localStorage throws", () => {
    mockLocalStorageFailure();

    expect(() => localeStore.setLocale("fr")).not.toThrow();
    expect(get(locale)).toBe("fr");
    expect(document.documentElement.getAttribute("lang")).toBe("fr");
  });
});
