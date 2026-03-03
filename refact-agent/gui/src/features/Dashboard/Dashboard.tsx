import React, { useCallback, useRef, useState } from "react";
import { StatsStrip } from "./components/StatsStrip/StatsStrip";
import { OpenSection } from "./components/OpenSection/OpenSection";
import { RecentSection } from "./components/RecentSection/RecentSection";
import { NavBar } from "./components/NavBar/NavBar";
import { useDashboardLayout } from "./hooks/useDashboardLayout";
import { useOpenTabsData } from "./hooks/useOpenTabsData";
import styles from "./Dashboard.module.css";

export const Dashboard: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const breakpoint = useDashboardLayout(containerRef);
  const openTabs = useOpenTabsData();
  const [expanded, setExpanded] = useState(false);

  const toggleExpand = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  const hasOpenTabs = openTabs.length > 0;

  return (
    <div
      ref={containerRef}
      className={styles.dashboard}
      data-breakpoint={breakpoint}
    >
      {/* Stats Strip — compact when RECENT is expanded */}
      <StatsStrip breakpoint={breakpoint} compact={expanded} />

      {/* Open tabs section — compact when RECENT is expanded */}
      {hasOpenTabs && (
        <OpenSection
          tabs={openTabs}
          breakpoint={breakpoint}
          compact={expanded}
        />
      )}

      {/* TODO: SetupBanner will go here when setup mode is implemented */}
      {/* TODO: BackgroundSection will go here when background tasks are implemented */}

      {/* Recent history — expandable */}
      <RecentSection
        breakpoint={breakpoint}
        expanded={expanded}
        onToggleExpand={toggleExpand}
      />

      {/* Bottom nav bar */}
      <NavBar />
    </div>
  );
};
