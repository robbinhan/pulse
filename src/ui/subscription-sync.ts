import { Clipboard, Storage } from "nsh";
import { Core } from "theysay/native";
import { applyParsed, notify, profile, type SyncPrefs } from "./model";

export const SYNC_KEY = "theysay.sync.v1";

export interface SyncConfig {
  url: string;
  token: string;
  pullOnOpen: boolean;
}

const emptyConfig = (): SyncConfig => ({ url: "", token: "", pullOnOpen: false });

export async function loadSync(): Promise<SyncConfig> {
  const raw = await Storage.getItem(SYNC_KEY);
  if (!raw) return emptyConfig();
  try {
    const parsed = JSON.parse(raw) as Partial<SyncConfig>;
    return {
      url: typeof parsed.url === "string" ? parsed.url : "",
      token: typeof parsed.token === "string" ? parsed.token : "",
      pullOnOpen: parsed.pullOnOpen === true,
    };
  } catch {
    return emptyConfig();
  }
}

export async function saveSync(config: SyncConfig) {
  await Storage.setItem(
    SYNC_KEY,
    JSON.stringify({
      url: config.url.trim(),
      token: config.token.trim(),
      pullOnOpen: config.pullOnOpen,
    }),
  );
}

function checkUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) return "";
  if (!trimmed.startsWith("https://") || trimmed.length > 500 || /\s/.test(trimmed)) return "同步地址要是 https。";
  return "";
}

function headers(token: string, json: boolean) {
  const out: Record<string, string> = { Accept: "application/json" };
  if (json) out["Content-Type"] = "application/json";
  const secret = token.trim();
  if (secret) out.Authorization = `Bearer ${secret}`;
  return out;
}

/** Pulls the remote list and unions it into this device. */
export async function pullSubscriptions(config: SyncConfig): Promise<string> {
  const bad = checkUrl(config.url);
  if (bad || !config.url.trim()) return bad || "先填写同步地址。";
  let text = "";
  try {
    const response = await fetch(config.url.trim(), { headers: headers(config.token, false) });
    if (!response.ok) return `云端返回 ${response.status}。`;
    text = await response.text();
  } catch {
    return "没有连上这个地址。";
  }
  let env;
  try {
    env = applyParsed(JSON.parse(Core.mergeSubscriptions(text)));
  } catch {
    return "云端的内容不是订阅列表。";
  }
  if (!env.ok) return env.error || "没有合并。";
  applyParsed(JSON.parse(Core.noteSync()));
  return env.applied ? "已合并订阅列表。" : "订阅列表没有新的。";
}

/** Uploads this device's list. A server that rejects the write leaves a copy on the clipboard. */
export async function pushSubscriptions(config: SyncConfig): Promise<string> {
  const body = Core.exportSubscriptions();
  const bad = checkUrl(config.url);
  if (bad || !config.url.trim()) {
    await Clipboard.writeText(body);
    return bad || "还没有同步地址。订阅列表已复制。";
  }
  const head = headers(config.token, true);
  const url = config.url.trim();
  let uploaded = false;
  try {
    const put = await fetch(url, { method: "PUT", headers: head, body });
    uploaded = put.ok;
    if (!uploaded) {
      const post = await fetch(url, { method: "POST", headers: head, body });
      uploaded = post.ok;
    }
  } catch {
    uploaded = false;
  }
  if (!uploaded) {
    await Clipboard.writeText(body);
    return "这个地址不接受上传。订阅列表已复制，可以贴到你的文件里。";
  }
  applyParsed(JSON.parse(Core.noteSync()));
  return "订阅列表已上传。";
}

export function syncPrefs(): SyncPrefs {
  return (
    profile().sync ?? {
      mode: "off",
      url: "",
      token: "",
      pullOnOpen: false,
      lastSyncMs: 0,
    }
  );
}

/** Copies an older local URL into the profile as https mode. */
export async function ensureSyncPrefs(): Promise<SyncPrefs> {
  const current = syncPrefs();
  if (current.mode === "https" || current.mode === "icloud") return current;
  if (current.url.trim() && current.mode !== "https") {
    const next = { ...current, mode: "https" };
    applyParsed(JSON.parse(Core.setSync(next.mode, next.url, next.token, next.pullOnOpen)));
    return syncPrefs();
  }
  const legacy = await loadSync();
  if (!legacy.url.trim()) return current;
  applyParsed(JSON.parse(Core.setSync("https", legacy.url, legacy.token, legacy.pullOnOpen)));
  return syncPrefs();
}

let pulled = false;

export async function pullSyncIfEnabled() {
  if (pulled) return;
  pulled = true;
  const config = await ensureSyncPrefs();
  if (!config.pullOnOpen) return;
  if (config.mode === "https") {
    notify(await pullSubscriptions(config));
    return;
  }
  if (config.mode !== "icloud") return;
  const status = JSON.parse(Core.icloudStatus()) as { available?: boolean };
  if (!status.available) return;
  const env = applyParsed(JSON.parse(Core.icloudPull()));
  notify(env.ok ? (env.applied ? "已从 iCloud 合并。" : "订阅列表没有新的。") : env.error || "没有从 iCloud 合并。");
}
