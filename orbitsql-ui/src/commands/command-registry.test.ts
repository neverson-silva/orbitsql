import { describe, expect, it } from "vitest";
import { COMMAND_IDS } from "./command-model";
import {
  executeCommand,
  isCommandAvailable,
  registerCommandHandlers,
} from "./command-registry";

describe("command registry", () => {
  it("executes a registered command and restores the previous handler", async () => {
    const calls: string[] = [];
    const firstCleanup = registerCommandHandlers({
      [COMMAND_IDS.viewQuickOpen]: () => {
        calls.push("first");
      },
    });
    const secondCleanup = registerCommandHandlers({
      [COMMAND_IDS.viewQuickOpen]: () => {
        calls.push("second");
      },
    });

    await executeCommand(COMMAND_IDS.viewQuickOpen);
    secondCleanup();
    await executeCommand(COMMAND_IDS.viewQuickOpen);
    firstCleanup();

    expect(calls).toEqual(["second", "first"]);
    expect(isCommandAvailable(COMMAND_IDS.viewQuickOpen)).toBe(false);
  });
});
