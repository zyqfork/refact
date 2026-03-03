import React, { useCallback } from "react";
import { Text } from "@radix-ui/themes";
import { useAppDispatch } from "../../../../hooks";
import { switchToThread } from "../../../Chat/Thread";
import { popBackTo, push } from "../../../Pages/pagesSlice";
import { useGetChatModesQuery } from "../../../../services/refact/chatModes";
import { OpenTabCard } from "./OpenTabCard";
import type { OpenTabData, DashboardBreakpoint } from "../../types";
import styles from "./OpenSection.module.css";

type OpenSectionProps = {
  tabs: OpenTabData[];
  breakpoint: DashboardBreakpoint;
  compact?: boolean;
};

export const OpenSection: React.FC<OpenSectionProps> = ({
  tabs,
  breakpoint,
  compact,
}) => {
  const dispatch = useAppDispatch();
  const { data: modesData } = useGetChatModesQuery(undefined);

  const handleTabClick = useCallback(
    (tabId: string) => {
      dispatch(switchToThread({ id: tabId }));
      dispatch(popBackTo({ name: "history" }));
      dispatch(push({ name: "chat" }));
    },
    [dispatch],
  );

  if (tabs.length === 0) return null;

  if (compact) {
    return (
      <div className={styles.compact}>
        <Text size="1" color="gray">
          📌 {tabs.length} open
        </Text>
      </div>
    );
  }

  return (
    <div className={styles.section}>
      <Text size="1" weight="bold" color="gray" className={styles.label}>
        📌 OPEN
      </Text>
      <div
        className={styles.grid}
        data-breakpoint={breakpoint}
      >
        {tabs.map((tab) => {
          const modeInfo = modesData?.modes.find((m) => m.id === tab.mode);
          const modeLabel = modeInfo?.title ?? tab.mode;
          return (
            <OpenTabCard
              key={tab.id}
              tab={tab}
              breakpoint={breakpoint}
              modeLabel={modeLabel}
              onClick={() => handleTabClick(tab.id)}
            />
          );
        })}
      </div>
    </div>
  );
};
