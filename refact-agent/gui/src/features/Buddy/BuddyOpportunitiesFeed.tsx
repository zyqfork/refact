import React from "react";
import { Text } from "@radix-ui/themes";
import { BuddyOpportunityCard } from "./BuddyOpportunityCard";
import { useBuddyOpportunities } from "./hooks/useBuddyOpportunities";
import styles from "./BuddyOpportunitiesFeed.module.css";

export const BuddyOpportunitiesFeed: React.FC = () => {
  const { unread } = useBuddyOpportunities();

  return (
    <div className={styles.feed} data-testid="buddy-opportunities-feed">
      <div className={styles.header}>
        <Text size="1" weight="bold" color="gray" className={styles.label}>
          OPPORTUNITIES
        </Text>
        {unread.length > 0 && (
          <Text size="1" color="gray" className={styles.count}>
            {unread.length}
          </Text>
        )}
      </div>
      {unread.length === 0 ? (
        <Text size="1" className={styles.empty}>
          No opportunities right now.
        </Text>
      ) : (
        <div
          className={styles.list}
          role="list"
          aria-label="Buddy opportunities"
        >
          {unread.map((opp) => (
            <div key={opp.id} className={styles.item} role="listitem">
              <BuddyOpportunityCard opportunity={opp} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
