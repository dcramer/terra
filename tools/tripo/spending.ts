// The token stats (tools/token-stats) count Tripo credits beside Claude tokens.

import { recordSpend, recordedSpends, spendId } from "../token-stats/report.mjs";
import { report } from "./cli.ts";
import { type Studio, StudioError, log } from "./studio.ts";

// Notes the account's charges for the token stats from Tripo's own history, so web-app work
// counts too and amounts are exact; report.mjs sends them. Each operation is one entry by its
// id, so noting it again is harmless. Pending charges wait for the next run; cancelled and
// failed ones cost nothing.
export async function noteSpending(client: Studio, everything = false, quiet = false): Promise<void> {
  try {
    const charges = new Map<string, { credits: number; day: string }>();
    for (let page = 1; ; page++) {
      const data = await client.call("GET", "/v2/studio/txn/records", undefined, { page_num: page, page_size: 100 });
      for (const c of chargesIn(data.records ?? [])) {
        // One operation can be billed in parts, as two records.
        const known = charges.get(c.operation);
        charges.set(c.operation, { credits: (known?.credits ?? 0) + c.credits, day: c.day });
      }
      if (!(everything && data.have_more)) break;
    }
    const recorded = recordedSpends();
    let noted = 0;
    for (const [op, c] of charges) {
      if (c.credits <= (recorded.get(spendId("tripo-studio", op)) ?? 0)) continue;
      recordSpend("tripo-studio", op, c.credits, c.day);
      noted++;
    }
    if (!quiet) report({ operations: charges.size, credits: [...charges.values()].reduce((s, c) => s + c.credits, 0), newly_noted: noted });
  } catch (err) {
    // The stats must never stop a model being made.
    if (!quiet) throw new StudioError(`noting spending failed: ${(err as Error).message}`);
    log(`(could not note Tripo spending for the token stats: ${(err as Error).message})`);
  }
}

// The fulfilled charges in a page of Tripo's history, by operation.
export function chargesIn(records: any[]): { operation: string; credits: number; day: string }[] {
  return records
    .filter((r) => r.txn_type === "charge" && r.status === "fulfilled" && /[1-9a-f]/.test(r.operator_id ?? ""))
    .map((r) => ({ operation: r.operator_id, credits: Number(r.amount) || 0, day: String(r.create_time ?? "").slice(0, 10) }));
}
