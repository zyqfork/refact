import React, { useMemo } from "react";
import { Text } from "@radix-ui/themes";
import { useGetStatsSummaryQuery } from "../../../../services/refact/stats";
import { SparklineChart } from "./SparklineChart";
import { TokenDonut } from "./TokenDonut";
import { SuccessGauge } from "./SuccessGauge";
import { ModelBars } from "./ModelBars";
import { formatTokenCount } from "../../../StatsDashboard/utils/formatters";
import type { DashboardBreakpoint } from "../../types";
import styles from "./StatsStrip.module.css";

type StatsStripProps = {
  breakpoint: DashboardBreakpoint;
  compact?: boolean;
};

function get7DaysAgo(): string {
  const d = new Date();
  d.setDate(d.getDate() - 7);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

export const StatsStrip: React.FC<StatsStripProps> = ({
  breakpoint,
  compact,
}) => {
  const from = useMemo(() => get7DaysAgo(), []);
  const { data, isLoading } = useGetStatsSummaryQuery({ from });

  if (isLoading || !data) {
    return (
      <div className={styles.strip}>
        <Text size="1" color="gray">Loading stats...</Text>
      </div>
    );
  }

  const { totals, by_day, by_model } = data;

  if (compact) {
    const cost = totals.total_cost_usd != null
      ? `$${totals.total_cost_usd.toFixed(2)}`
      : `${formatTokenCount(totals.total_tokens)} tok`;
    const successRate = totals.total_calls > 0
      ? `${Math.round((totals.successful_calls / totals.total_calls) * 100)}%`
      : "—";
    return (
      <div className={styles.stripCompact}>
        <Text size="1" color="gray">
          {totals.total_conversations} chats · {cost} · {successRate}
        </Text>
      </div>
    );
  }

  return (
    <div className={styles.strip} data-breakpoint={breakpoint}>
      <div className={styles.card}>
        <Text size="1" color="gray" className={styles.cardLabel}>7-day activity</Text>
        <SparklineChart days={by_day} />
      </div>
      <div className={styles.card}>
        <Text size="1" color="gray" className={styles.cardLabel}>Tokens</Text>
        <TokenDonut
          prompt={totals.total_prompt_tokens}
          completion={totals.total_completion_tokens}
          cache={totals.total_cache_read_tokens + totals.total_cache_creation_tokens}
        />
      </div>
      <div className={styles.card}>
        <Text size="1" color="gray" className={styles.cardLabel}>Success</Text>
        <SuccessGauge
          successful={totals.successful_calls}
          total={totals.total_calls}
        />
      </div>
      {breakpoint === "wide" && (
        <div className={styles.card}>
          <Text size="1" color="gray" className={styles.cardLabel}>Models</Text>
          <ModelBars models={by_model} />
        </div>
      )}
    </div>
  );
};


