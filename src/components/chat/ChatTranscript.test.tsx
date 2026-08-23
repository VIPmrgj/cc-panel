import { describe, expect, it } from "vitest";
import { extractDisplayedUserPrompt } from "./promptDisplay";

describe("extractDisplayedUserPrompt", () => {
  it("shows only the user prompt while preserving XML transport content", () => {
    const wrapped = [
      '<cc-panel-prompt version="1">',
      "  <selected-skills>",
      '    <skill id="demo">hidden skill body</skill>',
      "  </selected-skills>",
      '  <user-prompt variant="original">',
      "    这是一个测试对话，进行一些权限的提交，然后进行一些测试类对话",
      "    第二行 &lt;保持&gt; &amp; 原样",
      "  </user-prompt>",
      "  <attachments>",
      '    <attachment name="secret.txt">hidden attachment</attachment>',
      "  </attachments>",
      "</cc-panel-prompt>",
    ].join(String.fromCharCode(10));

    expect(extractDisplayedUserPrompt(wrapped)).toBe(
      [
        "这是一个测试对话，进行一些权限的提交，然后进行一些测试类对话",
        "第二行 <保持> & 原样",
      ].join(String.fromCharCode(10)),
    );
    expect(wrapped).toContain("<selected-skills>");
    expect(wrapped).toContain("<attachments>");
  });

  it("leaves legacy plain messages unchanged", () => {
    expect(extractDisplayedUserPrompt("普通历史消息")).toBe("普通历史消息");
  });
});
