// Attaching files to the composer, from the file picker, a drop or the clipboard.

import { open } from "@tauri-apps/plugin-dialog";
import { useChat } from "./chatStore";
import { ipc, toAppError } from "./ipc";
import { errorMessage } from "./strings";

/** Ingests dropped or picked paths and stages whatever succeeded. */
export async function attachPaths(paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  try {
    const result = await ipc.attachFiles(paths);
    useChat.getState().addAttachments(result.attachments, result.failures);
  } catch (e) {
    useChat.getState().addAttachments([], [errorMessage(toAppError(e).code)]);
  }
}

/** Opens the system file picker. Any file type is allowed. */
export async function pickFiles(): Promise<void> {
  const picked = await open({ multiple: true, directory: false });
  if (!picked) return;
  await attachPaths(Array.isArray(picked) ? picked : [picked]);
}

function readDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(reader.error ?? new Error("could not read the file"));
    reader.readAsDataURL(file);
  });
}

function pastedName(file: File): string {
  if (file.name) return file.name;
  const ext = file.type.split("/")[1]?.split("+")[0] || "bin";
  const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
  return `pasted-${stamp}.${ext}`;
}

/** Stages one clipboard file (an image, usually) that has no path on disk. */
async function attachFile(file: File): Promise<void> {
  const chat = useChat.getState();
  try {
    const dataUrl = await readDataUrl(file);
    const attachment = await ipc.attachBytes(pastedName(file), file.type || null, dataUrl);
    chat.addAttachments([attachment]);
  } catch (e) {
    chat.addAttachments([], [toAppError(e).detail]);
  }
}

/**
 * Handles a paste into the composer. Files become attachments, and text longer
 * than `pasteAsFileChars` is attached as a text file instead of filling the
 * composer. Returns true when the paste was handled here.
 */
export async function attachFromPaste(data: DataTransfer, pasteAsFileChars: number): Promise<boolean> {
  const files = Array.from(data.files);
  if (files.length > 0) {
    for (const file of files) await attachFile(file);
    return true;
  }
  const text = data.getData("text/plain");
  if (pasteAsFileChars > 0 && text.length > pasteAsFileChars) {
    const chat = useChat.getState();
    try {
      chat.addAttachments([await ipc.attachText("pasted-text.txt", text)]);
    } catch (e) {
      chat.addAttachments([], [toAppError(e).detail]);
      return false;
    }
    return true;
  }
  return false;
}
