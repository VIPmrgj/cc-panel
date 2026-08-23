/**
 * The transport intentionally keeps the complete CC Panel composition. The
 * transcript only exposes the user-facing part of that composition.
 */
export function extractDisplayedUserPrompt(content: string): string {
  const rootStart = content.indexOf("<cc-panel-prompt");
  if (rootStart < 0) return content;
  const rootEnd = content.indexOf(">", rootStart);
  if (rootEnd < 0) return content;

  const userStart = content.indexOf("<user-prompt", rootEnd + 1);
  if (userStart < 0) return content;
  const userEnd = content.indexOf(">", userStart);
  const userClose = content.indexOf("</user-prompt>", userEnd + 1);
  if (userEnd < 0 || userClose < 0) return content;

  const body = content
    .slice(userEnd + 1, userClose)
    .replace(/^\r?\n/, "")
    .replace(/\r?\n[ \t]*$/, "")
    .split(/\r?\n/)
    .map((line) => (line.startsWith("    ") ? line.slice(4) : line))
    .join("\n");

  return decodeXmlText(body);
}

function decodeXmlText(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, "&");
}
