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
import { LocalStorage, STORAGE_KEYS } from "./localStorage.service";

describe("LocalStorage service", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    window.localStorage.clear();
  });

  it("returns the default value when localStorage.getItem throws", () => {
    vi.spyOn(window.localStorage.__proto__, "getItem").mockImplementation(
      () => {
        throw new Error("storage unavailable");
      },
    );

    expect(
      LocalStorage.get(STORAGE_KEYS.SELECTED_WORKFLOW_ID, null),
    ).toBeNull();
  });

  it("does not throw when localStorage.setItem throws", () => {
    vi.spyOn(window.localStorage.__proto__, "setItem").mockImplementation(
      () => {
        throw new Error("storage unavailable");
      },
    );

    expect(() => LocalStorage.set("theme", "dark")).not.toThrow();
  });
});
