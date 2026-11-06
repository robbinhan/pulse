import { Core } from "theysay/native";
import { profile } from "./model";

function localDay(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/** Once, after launch, if the local hour has reached the setting. Failures stay quiet. */
export function maybeMorningBriefing() {
  if (profile().settings.briefingAuto === false) return;
  const hour = profile().settings.briefingHour ?? 8;
  const now = new Date();
  if (now.getHours() < hour) return;
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  const end = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  void Promise.resolve(Core.makeBriefing(start.getTime(), end.getTime(), localDay(now), false)).catch(() => {});
}
